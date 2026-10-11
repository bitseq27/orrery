//! Telemetry computation for the left column (R11) — live from the ephemeris buffer
//! (here: direct evaluation, worker thread arrives with the M2 belt/cloud work).

use orrery_astro::kepler::{elements_from_state, Elements};
use orrery_astro::moons::{moon_defs, moon_props};
use orrery_astro::planets::{planet_pos_ecl, planet_state, planet_state_ecl, PlanetId, PLANET_IDS, PLANET_NAMES};
use orrery_astro::population::population;
use orrery_astro::probes::{probe_state, probes};
use orrery_astro::small_bodies::{small_bodies, Kind};
use orrery_astro::AU_KM;

#[derive(Clone, Debug)]
pub struct Telemetry {
    /// element uncertainty (exoplanets, §5.2 honesty row)
    pub a_err: Option<f64>,
    pub e_err: Option<f64>,
    pub name: String,
    pub kind: &'static str,
    pub mass_kg: f64,
    pub radius_km: f64,
    pub color: egui_color,
    pub dist_star_au: f64,
    pub vel_kms: f64,
    pub dist_geo_au: f64,
    /// distance to parent body (moons); 0 otherwise
    pub dist_parent_au: f64,
    pub phase_deg: f64,
    pub el: Elements,
    pub pos_icrs: [f64; 3],
    pub vel_icrs: [f64; 3],
    // ---- session-4 additions: physical / temporal context -----------------
    /// orbital period [d] (authoritative source per body class)
    pub period_d: f64,
    /// temperature [K] + provenance note ("MEAN", "1-BAR", "EQ@r", …)
    pub temp_k: Option<f64>,
    pub temp_note: &'static str,
    /// bulk composition class (static tables / SBDB spectral class)
    pub composition: &'static str,
    /// sidereal rotation period [d]; negative = retrograde; None = unknown
    pub rot_d: Option<f64>,
    pub rot_note: &'static str,
    /// formation age [Gyr]; None = unknown
    pub age_gyr: Option<f64>,
    /// JPL potentially-hazardous (impact risk) / near-Earth-object flags
    pub pha: bool,
    pub neo: bool,
    /// orbit-class label (small bodies: APOLLO NEA, MAIN BELT, …)
    pub class: String,
    // ---- session-5: spacecraft rows (empty/0 for natural bodies) ------------
    pub agency: &'static str,
    pub launched: u32,
    pub mission: &'static str,
    pub status: &'static str,
}

pub type egui_color = [u8; 3];

/// Flat HMI colour tokens per planet (§7: catalog colour, no PBR).
pub const PLANET_RGB: [[u8; 3]; 9] = [
    [168, 168, 168], // MERCURY
    [230, 200, 122], // VENUS
    [95, 165, 224],  // EARTH
    [210, 96, 74],   // MARS
    [200, 160, 110], // JUPITER
    [216, 192, 138], // SATURN
    [127, 212, 212], // URANUS
    [79, 110, 216],  // NEPTUNE
    [176, 152, 128], // PLUTO
];

pub const PLANET_MASS_KG: [f64; 9] = [
    3.3011e23, 4.8675e24, 5.9724e24, 6.4171e23, 1.8982e27, 5.6834e26, 8.6810e25, 1.0241e26, 1.303e22,
];
/// Confirmed moon counts (IAU as of 2025: Saturn 274 after the 2025 batch),
/// for the MOONS readout row (§6.1 counting).
pub const KNOWN_MOONS: [u32; 9] = [0, 0, 1, 2, 95, 274, 28, 16, 5];

/// Planet masses in solar masses (mu for parent-relative moon elements).
pub const PLANET_MASS_MSUN: [f64; 9] = [
    1.6601e-7, 2.4478e-6, 3.0035e-6, 3.2271e-7, 9.5479e-4, 2.8588e-4, 4.3662e-5, 5.1514e-5, 7.69e-9,
];

pub const PLANET_RADIUS_KM: [f64; 9] = [
    2439.7, 6051.8, 6371.0, 3389.5, 69911.0, 58232.0, 25362.0, 24622.0, 1188.3,
];

pub fn planet_index(id: PlanetId) -> usize {
    PLANET_IDS.iter().position(|p| *p == id).unwrap()
}

pub fn id_from_name(name: &str) -> Option<PlanetId> {
    PLANET_NAMES.iter().position(|n| *n == name).map(|i| PLANET_IDS[i])
}

/// Full live telemetry for a Sol-system planet at `jd` (TT).
/// Session-5 perf fix: evaluates VSOP87 for (planet, Earth) ONCE each — the old
/// path called planet_state + geocentric_state + 2× planet_state_ecl, i.e. ~15
/// full-series evaluations per frame while a planet was selected (frame dt is
/// clamped, so the fps drop literally slowed the animation).
pub fn telemetry_planet(id: PlanetId, jd: f64) -> Telemetry {
    let i = planet_index(id);
    let se = planet_state_ecl(id, jd); // ecliptic, pos+vel — VSOP evaluated ONCE
    let ee = planet_state_ecl(PlanetId::Earth, jd);
    let s = orrery_astro::kepler::State {
        pos: orrery_astro::frames::ecl_to_icrs(se.pos),
        vel: orrery_astro::frames::ecl_to_icrs(se.vel),
    };
    let el = elements_from_state(&s, 1.0);
    // phase angle Sun–body–Earth (from the ecliptic states already computed)
    let to_sun = [-se.pos[0], -se.pos[1], -se.pos[2]];
    let to_earth = [ee.pos[0] - se.pos[0], ee.pos[1] - se.pos[1], ee.pos[2] - se.pos[2]];
    let phase = ang(&to_sun, &to_earth).to_degrees();
    let dist_geo = ((se.pos[0] - ee.pos[0]).powi(2)
        + (se.pos[1] - ee.pos[1]).powi(2)
        + (se.pos[2] - ee.pos[2]).powi(2))
    .sqrt();
    let (temp, tnote, comp, rot, age, _) = crate::body_data::planet_tables(id);
    Telemetry {
        name: PLANET_NAMES[i].to_string(),
        kind: if id == PlanetId::Pluto { "DWARF PLANET" } else { "PLANET" },
        a_err: None,
        e_err: None,
        mass_kg: PLANET_MASS_KG[i],
        radius_km: PLANET_RADIUS_KM[i],
        color: PLANET_RGB[i],
        dist_star_au: s.r_au(),
        vel_kms: s.speed_kms(),
        dist_geo_au: dist_geo,
        dist_parent_au: 0.0,
        phase_deg: phase,
        el,
        pos_icrs: s.pos,
        vel_icrs: s.vel,
        period_d: orrery_astro::planets::period_days(id),
        temp_k: Some(temp),
        temp_note: tnote,
        composition: comp,
        rot_d: Some(rot),
        rot_note: "",
        age_gyr: Some(age),
        pha: false,
        neo: false,
        class: String::new(),
        agency: "",
        launched: 0,
        mission: "",
        status: "",
    }
}

fn ang(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let na = (a[0].powi(2) + a[1].powi(2) + a[2].powi(2)).sqrt();
    let nb = (b[0].powi(2) + b[1].powi(2) + b[2].powi(2)).sqrt();
    (dot / (na * nb)).clamp(-1.0, 1.0).acos()
}

/// Distance in km for readouts.
pub fn au_km(au: f64) -> f64 {
    au * AU_KM
}

/// Exoplanet telemetry (R5/R11): two-body about the host star (§5.2).
pub fn telemetry_exo(cat: &crate::catalog::Catalog, sys: usize, name: &str, jd: f64) -> Option<Telemetry> {
    let s = cat.systems.get(sys)?;
    let pi = s.planets.iter().position(|p| p.name == name)?;
    let p = &s.planets[pi];
    let el = cat.elements(sys, pi);
    let st = el.state(jd);
    let cols: [[u8; 3]; 10] = [
        [95, 165, 224], [210, 96, 74], [230, 200, 122], [127, 212, 212], [200, 160, 110],
        [168, 168, 168], [79, 110, 216], [216, 192, 138], [176, 152, 128], [210, 212, 212],
    ];
    // live equilibrium temperature from the host star (Bond albedo 0.3)
    let teq = s.host.teff_k.zip(s.host.radius_rsun).map(|(teff, rs)| {
        teff * (rs * 0.004_650_47 / (2.0 * p.a_au.max(1e-6))).sqrt() * 0.9036
    });
    Some(Telemetry {
        a_err: p.a_err,
        e_err: p.e_err,
        name: p.name.clone(),
        kind: "EXOPLANET",
        mass_kg: p.mass_mj.map(|m| m * 1.898e27).unwrap_or(0.0),
        radius_km: p.radius_rj.map(|r| r * 69911.0).unwrap_or(0.0),
        color: cols[pi % 10],
        dist_star_au: st.r_au(),
        vel_kms: st.speed_kms(),
        dist_geo_au: 0.0,
        dist_parent_au: 0.0,
        phase_deg: 0.0,
        el,
        pos_icrs: st.pos,
        vel_icrs: st.vel,
        period_d: p.period_d,
        temp_k: teq,
        temp_note: "EQ(0.3A) CALC",
        composition: crate::body_data::exo_composition(p.radius_rj, p.mass_mj),
        rot_d: None,
        rot_note: "",
        age_gyr: None, // host age not carried in the TAP snapshot (§5.2)
        pha: false,
        neo: false,
        class: String::new(),
        agency: "",
        launched: 0,
        mission: "",
        status: "",
    })
}

/// Resolve any selectable body name → live telemetry (planets, moons, asteroids, comets).
pub fn resolve(name: &str, jd: f64) -> Option<Telemetry> {
    if let Some(id) = id_from_name(name) {
        return Some(telemetry_planet(id, jd));
    }
    if let Some(m) = moon_defs().iter().find(|m| m.name == name) {
        let s = orrery_astro::moons::moon_state(m, jd);
        // parent-RELATIVE state: heliocentric osculating elements of a moon are
        // meaningless (hyperbolic garbage → NaN periods → inf scope bounds)
        let pe = planet_state_ecl(m.parent, jd);
        let me = orrery_astro::frames::icrs_to_ecl(s.pos);
        let mv = orrery_astro::frames::icrs_to_ecl(s.vel);
        let rel = orrery_astro::kepler::State {
            pos: orrery_astro::frames::ecl_to_icrs([me[0] - pe.pos[0], me[1] - pe.pos[1], me[2] - pe.pos[2]]),
            vel: orrery_astro::frames::ecl_to_icrs([mv[0] - pe.vel[0], mv[1] - pe.vel[1], mv[2] - pe.vel[2]]),
        };
        let mu = PLANET_MASS_MSUN[planet_index(m.parent)];
        let el = elements_from_state(&rel, mu);
        let rel_speed = (rel.vel[0].powi(2) + rel.vel[1].powi(2) + rel.vel[2].powi(2)).sqrt() * orrery_astro::AUPERDAY_KMS;
        let (mass, rad) = moon_props(name);
        let temp = crate::body_data::moon_mean_t_k(name);
        let tnote = if temp.is_some() { "MEAN" } else { "EQ@r CALC" };
        let temp = temp.or(Some(crate::body_data::equilibrium_t_k(s.r_au().max(0.05), 0.1)));
        return Some(Telemetry {
            name: name.to_string(),
            kind: "MOON",
            a_err: None,
            e_err: None,
            mass_kg: mass,
            radius_km: rad,
            color: [200, 200, 200],
            dist_star_au: s.r_au(),
            vel_kms: rel_speed,
            dist_geo_au: {
                let e = planet_state(PlanetId::Earth, jd);
                ((e.pos[0]-s.pos[0]).powi(2)+(e.pos[1]-s.pos[1]).powi(2)+(e.pos[2]-s.pos[2]).powi(2)).sqrt()
            },
            dist_parent_au: rel.r_au(),
            phase_deg: 0.0,
            el,
            pos_icrs: s.pos,
            vel_icrs: s.vel,
            period_d: m.period_d,
            temp_k: temp,
            temp_note: tnote,
            composition: crate::body_data::moon_comp(name),
            rot_d: orrery_astro::moons::moon_rot_days(name),
            rot_note: if name == "HYPERION" { "CHAOTIC" } else { "SYNC" },
            age_gyr: Some(crate::body_data::moon_age_gyr(name)),
            pha: false,
            neo: false,
            class: String::new(),
            agency: "",
            launched: 0,
            mission: "",
            status: "",
        });
    }
    if let Some(b) = small_bodies()
        .iter()
        .chain(population().iter())
        .find(|b| b.name == name)
    {
        let s = b.state(jd);
        let (mass, rad) = b.props();
        let albedo = if b.kind == Kind::Comet { 0.04 } else { 0.15 };
        return Some(Telemetry {
            name: name.to_string(),
            kind: if b.kind == Kind::Comet { "COMET" } else { "ASTEROID" },
            a_err: None,
            e_err: None,
            mass_kg: mass,
            radius_km: rad,
            color: if b.pha { [210, 59, 46] } else if b.kind == Kind::Comet { [127, 212, 212] } else { [200, 160, 110] },
            dist_star_au: s.r_au(),
            vel_kms: s.speed_kms(),
            dist_geo_au: {
                let e = planet_state(PlanetId::Earth, jd);
                ((s.pos[0] - e.pos[0]).powi(2) + (s.pos[1] - e.pos[1]).powi(2) + (s.pos[2] - e.pos[2]).powi(2)).sqrt()
            },
            dist_parent_au: 0.0,
            phase_deg: 0.0,
            el: b.el,
            pos_icrs: s.pos,
            vel_icrs: s.vel,
            period_d: b.el.period_days(),
            // live fast-rotator equilibrium temperature at the CURRENT r
            temp_k: Some(crate::body_data::equilibrium_t_k(s.r_au(), albedo)),
            temp_note: "EQ@r CALC",
            composition: b.composition(),
            rot_d: if b.rot_h > 0.0 { Some(b.rot_h / 24.0) } else { None },
            rot_note: "",
            age_gyr: Some(4.56), // solar-system solids (CAI formation)
            pha: b.pha,
            neo: b.neo,
            class: b.class_label().to_string(),
            agency: "",
            launched: 0,
            mission: "",
            status: "",
        });
    }
    // Sol itself — selectable session 4 (free-camera focus target)
    if name == crate::body_data::SUN {
        use crate::body_data::*;
        let e = planet_state(PlanetId::Earth, jd);
        return Some(Telemetry {
            name: "SOL".to_string(),
            kind: "STAR (G2V)",
            a_err: None,
            e_err: None,
            mass_kg: SUN_MASS_KG,
            radius_km: SUN_RADIUS_KM,
            color: [224, 168, 0],
            dist_star_au: 0.0,
            vel_kms: 0.0,
            dist_geo_au: e.r_au(),
            dist_parent_au: 0.0,
            phase_deg: 0.0,
            el: Elements { a: 0.0, e: 0.0, i: 0.0, omega: 0.0, w: 0.0, l0: 0.0, epoch: 0.0, n: 0.0, mu_scale: 1.0, rates: [0.0; 6] },
            pos_icrs: [0.0; 3],
            vel_icrs: [0.0; 3],
            period_d: 0.0, // galactic year ~230 Myr — not an orbit here
            temp_k: Some(SUN_TEFF_K),
            temp_note: "PHOTOSPHERE",
            composition: SUN_COMP,
            rot_d: Some(SUN_ROT_D),
            rot_note: "EQ",
            age_gyr: Some(SUN_AGE_GYR),
            pha: false,
            neo: false,
            class: "GALACTIC ORBIT ~230 MYR".to_string(),
            agency: "",
            launched: 0,
            mission: "",
            status: "",
        });
    }
    // ---- spacecraft (session 5): probes from the baked Horizons states -------
    if let Some(pr) = probes().iter().find(|p| p.name == name) {
        let s = probe_state(pr, jd);
        let vt = pr.vel_true;
        let speed = (vt[0] * vt[0] + vt[1] * vt[1] + vt[2] * vt[2]).sqrt() * orrery_astro::AUPERDAY_KMS;
        let e = planet_state(PlanetId::Earth, jd);
        let dist_geo = ((s.pos[0] - e.pos[0]).powi(2)
            + (s.pos[1] - e.pos[1]).powi(2)
            + (s.pos[2] - e.pos[2]).powi(2))
        .sqrt();
        let prop = if pr.parent_locked {
            "PARENT-LOCKED KEPLER"
        } else if pr.bound {
            "TWO-BODY KEPLER"
        } else {
            "LINEAR (HYPERBOLIC ESCAPE)"
        };
        return Some(Telemetry {
            name: pr.name.to_string(),
            kind: "PROBE",
            a_err: None,
            e_err: None,
            mass_kg: 0.0,
            radius_km: 0.0,
            color: [90, 230, 140],
            dist_star_au: s.r_au(),
            vel_kms: speed,
            dist_geo_au: dist_geo,
            dist_parent_au: 0.0,
            phase_deg: 0.0,
            el: pr.el,
            pos_icrs: s.pos,
            vel_icrs: s.vel,
            period_d: if pr.bound { pr.el.period_days() } else { 0.0 },
            temp_k: None,
            temp_note: "",
            composition: "SPACECRAFT",
            rot_d: None,
            rot_note: "",
            age_gyr: None,
            pha: false,
            neo: false,
            class: format!(
                "{}{}",
                if pr.active { "ACTIVE" } else { "SILENT" },
                if pr.status_note.is_empty() { String::new() } else { format!(" · {}", pr.status_note) }
            ),
            agency: pr.agency,
            launched: pr.launch_year,
            mission: pr.target,
            status: prop,
        });
    }
    None
}

/// Cheap per-sample evaluator for the live SCOPE trace (session-5 perf):
/// mode 0 = r(t) [position-only VSOP / Kepler], 1 = |v|(t), 2 = phase(t).
/// Mirrors `resolve()` but skips the full telemetry build; falls back to the
/// current value for anything unknown (flat line, same as before).
pub fn sample_plot(name: &str, jd: f64, mode: u8, fallback: f64) -> f64 {
    if name == crate::body_data::SUN {
        return 0.0;
    }
    if let Some(id) = id_from_name(name) {
        return match mode {
            1 => {
                let st = planet_state_ecl(id, jd);
                st.speed_kms()
            }
            2 => {
                let sp = planet_pos_ecl(id, jd);
                let se = planet_pos_ecl(PlanetId::Earth, jd);
                let to_sun = [-sp[0], -sp[1], -sp[2]];
                let to_earth = [se[0] - sp[0], se[1] - sp[1], se[2] - sp[2]];
                ang(&to_sun, &to_earth).to_degrees()
            }
            _ => {
                let p = planet_pos_ecl(id, jd);
                (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
            }
        };
    }
    if let Some(m) = moon_defs().iter().find(|m| m.name == name) {
        return match mode {
            // the mean-circle model IS a circle of radius a — exact, zero cost
            0 => m.a_km / AU_KM,
            _ => resolve(name, jd).map(|t| if mode == 1 { t.vel_kms } else { t.phase_deg }).unwrap_or(fallback),
        };
    }
    let body = small_bodies()
        .iter()
        .chain(population().iter())
        .find(|b| b.name == name);
    if let Some(b) = body {
        let st = b.el.state_ecl(jd);
        return match mode {
            1 => st.speed_kms(),
            2 => {
                let se = planet_pos_ecl(PlanetId::Earth, jd);
                let to_sun = [-st.pos[0], -st.pos[1], -st.pos[2]];
                let to_earth = [se[0] - st.pos[0], se[1] - st.pos[1], se[2] - st.pos[2]];
                ang(&to_sun, &to_earth).to_degrees()
            }
            _ => st.r_au(),
        };
    }
    if let Some(pr) = probes().iter().find(|p| p.name == name) {
        let st = probe_state(pr, jd);
        return match mode {
            1 => st.speed_kms(),
            _ => st.r_au(),
        };
    }
    fallback
}

#[cfg(test)]
mod perf_tests {
    use super::*;
    use orrery_astro::time::JD_J2000;

    /// Session-5 regression: a selected planet must NOT cost a per-frame
    /// ephemeris storm. 121 scope samples (one re-bake) must stay well under
    /// the frame budget at any sim rate — the cheap sampler evaluates only the
    /// position series, not the full telemetry build.
    #[test]
    fn scope_sampler_is_cheap() {
        let jd = JD_J2000 + 9700.0;
        let t0 = std::time::Instant::now();
        let mut acc = 0.0;
        for k in 0..121 {
            acc += sample_plot("MARS", jd + k as f64, 0, 1.0);
        }
        let el = t0.elapsed();
        assert!(acc > 0.0);
        // full telemetry for comparison must be the expensive path, not this
        assert!(el.as_millis() < 60, "121 samples took {el:?}");
    }

    #[test]
    fn probe_and_extended_resolution_works() {
        let jd = 2_461_324.5;
        let v1 = resolve("VOYAGER 1", jd).expect("probe resolves");
        assert_eq!(v1.kind, "PROBE");
        assert!((v1.dist_star_au - 172.0).abs() < 1.0, "V1 r {}", v1.dist_star_au);
        assert!(v1.agency == "NASA" && v1.launched == 1977);
        // extended REAL catalogue bodies resolve too (brightest-first prefix)
        let ext = &orrery_astro::population::population()[0];
        let t2 = resolve(ext.name, jd).expect("extended body resolves");
        assert_eq!(t2.name, ext.name);
        assert!(!t2.class.is_empty(), "class label expected");
    }
}
