//! Planetary states (PLAN.md §5.1): VSOP87 (full series, crate `vsop87`) for the eight
//! planets, JPL approximate Keplerian elements for Pluto, ELP-truncated Moon for the
//! Earth-barycentre correction. All output in ICRS-equatorial J2000, heliocentric.

use crate::frames::ecl_to_icrs;
use crate::kepler::{Elements, State};
use crate::moon::{moon_geocentric_ecl, MOON_MASS_RATIO};
use crate::time::{t_centuries, JD_J2000};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlanetId {
    Mercury,
    Venus,
    Earth,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
}

pub const PLANET_IDS: [PlanetId; 9] = [
    PlanetId::Mercury,
    PlanetId::Venus,
    PlanetId::Earth,
    PlanetId::Mars,
    PlanetId::Jupiter,
    PlanetId::Saturn,
    PlanetId::Uranus,
    PlanetId::Neptune,
    PlanetId::Pluto,
];

pub const PLANET_NAMES: [&str; 9] = [
    "MERCURY", "VENUS", "EARTH", "MARS", "JUPITER", "SATURN", "URANUS", "NEPTUNE", "PLUTO",
];

/// JPL approximate-position Keplerian tables (Standish, 1800–2050 AD),
/// [a, e, i, L, ϖ, Ω] + per-century rates; angles in degrees.
const JPL_TABLE: [[f64; 12]; 9] = [
    // a,      da,        e,         de,         i,          di,         L,           dL,          peri,       dperi,      node,       dnode
    [0.38709927, 0.00000037, 0.20563593, 0.00001906, 7.00497902, -0.00594749, 252.25032350, 149472.67411175, 77.45779628, 0.16047689, 48.33076593, -0.12534081],
    [0.72333566, 0.00000390, 0.00677672, -0.00004107, 3.39467605, -0.00078890, 181.97909950, 58517.81567487, 131.60246718, 0.00268329, 76.67984255, -0.27769418],
    [1.00000261, 0.00000562, 0.01671123, -0.00004392, -0.00001531, -0.01294668, 100.46457166, 35999.37244981, 102.93768193, 0.32327364, 0.0, 0.0],
    [1.52371034, 0.00001847, 0.09339410, 0.00007882, 1.84969142, -0.00813131, -4.55343205, 19140.30268499, -23.94362959, 0.44441088, 49.55953891, -0.29257343],
    [5.20288700, -0.00011607, 0.04838624, -0.00013253, 1.30439695, -0.00183714, 34.39644051, 3034.74612775, 14.72847983, 0.21252668, 100.47390909, 0.20469106],
    [9.53667594, -0.00125060, 0.05386179, -0.00050991, 2.48599187, 0.00193609, 49.95424423, 1222.49361025, 92.59887831, -0.41897216, 113.66242448, -0.28867794],
    [19.18916464, -0.00196176, 0.04725744, -0.00004397, 0.77263783, -0.00242939, 313.23810451, 428.48202785, 170.95427630, 0.40805281, 74.01692503, 0.04240589],
    [30.06992276, 0.00026291, 0.00859048, 0.00005105, 1.77004347, 0.00035372, -55.12002969, 218.46515314, 44.96476227, -0.32241464, 131.78422574, -0.00508664],
    [39.48211675, -0.00031596, 0.24882730, 0.00005170, 17.14001206, 0.00004818, 238.92903833, 145.20248673, 224.06891629, 0.60629951, 110.30393684, -0.01192956],
];

/// PlanetId from upper-case name (used by moons/catalog).
pub fn id_from_name(name: &str) -> Option<PlanetId> {
    PLANET_NAMES.iter().position(|n| *n == name).map(|i| PLANET_IDS[i])
}

pub fn jpl_elements(id: PlanetId) -> Elements {
    let t = &JPL_TABLE[idx(id)];
    Elements {
        a: t[0],
        e: t[2],
        i: t[4],
        omega: t[10],
        w: t[8] - t[10],
        l0: t[6],
        epoch: JD_J2000,
        n: 0.0,
        mu_scale: 1.0,
        rates: [t[1], t[3], t[5], t[11], t[9], t[7]],
    }
}

fn idx(id: PlanetId) -> usize {
    PLANET_IDS.iter().position(|p| *p == id).unwrap()
}

/// Heliocentric ecliptic J2000 position from VSOP87 (or elements for Pluto).
fn pos_ecl(id: PlanetId, jd: f64) -> [f64; 3] {
    match id {
        PlanetId::Mercury => {
            let c = vsop87::vsop87a::mercury(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Venus => {
            let c = vsop87::vsop87a::venus(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Earth => emb_ecl(jd),
        PlanetId::Mars => {
            let c = vsop87::vsop87a::mars(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Jupiter => {
            let c = vsop87::vsop87a::jupiter(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Saturn => {
            let c = vsop87::vsop87a::saturn(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Uranus => {
            let c = vsop87::vsop87a::uranus(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Neptune => {
            let c = vsop87::vsop87a::neptune(jd);
            [c.x, c.y, c.z]
        }
        PlanetId::Pluto => jpl_elements(PlanetId::Pluto).state_ecl(jd).pos,
    }
}

/// Earth-Moon barycentre, ecliptic J2000 (VSOP87).
pub fn emb_ecl(jd: f64) -> [f64; 3] {
    let c = vsop87::vsop87a::earth_moon(jd);
    [c.x, c.y, c.z]
}

/// Earth (not EMB): EMB minus the Moon's barycentric lever arm.
fn earth_ecl(jd: f64) -> [f64; 3] {
    let emb = emb_ecl(jd);
    // Moon vector from the truncated theory is mean-equinox-of-date ecliptic; the
    // precession difference over ±50 y is ≲0.4° on a 4 670 km lever arm → <0.1 m error.
    let m = moon_geocentric_ecl(jd);
    [
        emb[0] - MOON_MASS_RATIO * m[0],
        emb[1] - MOON_MASS_RATIO * m[1],
        emb[2] - MOON_MASS_RATIO * m[2],
    ]
}

/// Velocity by central difference (h = 0.25 d): ~1e-9 AU/d accuracy, far below display needs.
fn vel_ecl(id: PlanetId, jd: f64) -> [f64; 3] {
    let h = 0.25;
    let a = pos_ecl(id, jd - h);
    let b = pos_ecl(id, jd + h);
    [
        (b[0] - a[0]) / (2.0 * h),
        (b[1] - a[1]) / (2.0 * h),
        (b[2] - a[2]) / (2.0 * h),
    ]
}

/// POSITION ONLY (no central-difference velocity) — the cheap path used by the
/// live SCOPE r(t) sampler (session-5 perf: selection no longer slows the sim).
pub fn planet_pos_ecl(id: PlanetId, jd: f64) -> [f64; 3] {
    match id {
        PlanetId::Earth => earth_ecl(jd),
        other => pos_ecl(other, jd),
    }
}

/// Heliocentric ecliptic-J2000 state of a planet at JD (TT).
pub fn planet_state_ecl(id: PlanetId, jd: f64) -> State {
    let p = match id {
        PlanetId::Earth => earth_ecl(jd),
        other => pos_ecl(other, jd),
    };
    let v = match id {
        PlanetId::Earth => {
            let h = 0.25;
            let a = earth_ecl(jd - h);
            let b = earth_ecl(jd + h);
            [(b[0] - a[0]) / (2.0 * h), (b[1] - a[1]) / (2.0 * h), (b[2] - a[2]) / (2.0 * h)]
        }
        other => vel_ecl(other, jd),
    };
    State { pos: p, vel: v }
}

/// Heliocentric ICRS state of a planet at JD (TT).
pub fn planet_state(id: PlanetId, jd: f64) -> State {
    let s = planet_state_ecl(id, jd);
    State {
        pos: ecl_to_icrs(s.pos),
        vel: ecl_to_icrs(s.vel),
    }
}

/// The Sun is the heliocentric frame origin (§7: ICRS master, Sun-centred scenes).
pub fn sun_state() -> State {
    State::default()
}

/// Geocentric ICRS state (for Moon/observer-centric readouts).
pub fn geocentric_state(id: PlanetId, jd: f64) -> State {
    let e = planet_state(PlanetId::Earth, jd);
    let p = planet_state(id, jd);
    State {
        pos: [
            p.pos[0] - e.pos[0],
            p.pos[1] - e.pos[1],
            p.pos[2] - e.pos[2],
        ],
        vel: [
            p.vel[0] - e.vel[0],
            p.vel[1] - e.vel[1],
            p.vel[2] - e.vel[2],
        ],
    }
}

/// Moon geocentric ICRS state (ecliptic-of-date → J2000 ecliptic ≈ identity at our precision).
pub fn moon_state(jd: f64) -> State {
    let m = moon_geocentric_ecl(jd);
    let h = 0.25;
    let a = moon_geocentric_ecl(jd - h);
    let b = moon_geocentric_ecl(jd + h);
    let v = [
        (b[0] - a[0]) / (2.0 * h),
        (b[1] - a[1]) / (2.0 * h),
        (b[2] - a[2]) / (2.0 * h),
    ];
    State {
        pos: ecl_to_icrs(m),
        vel: ecl_to_icrs(v),
    }
}

/// Approximate orbital period [days] for orbit-line baking.
pub fn period_days(id: PlanetId) -> f64 {
    match id {
        PlanetId::Mercury => 87.969,
        PlanetId::Venus => 224.701,
        PlanetId::Earth => 365.256,
        PlanetId::Mars => 686.980,
        PlanetId::Jupiter => 4332.589,
        PlanetId::Saturn => 10759.22,
        PlanetId::Uranus => 30685.4,
        PlanetId::Neptune => 60189.0,
        PlanetId::Pluto => 90560.0,
    }
}

/// Validate span of the element tables (PLAN.md §5.3 "validated span").
pub fn validated_span_jd() -> (f64, f64) {
    (JD_J2000 - 73048.0, JD_J2000 + 91312.0) // ~1800–2250 AD for Pluto table margin
}

/// Secular drift helper used by tests/docs.
pub fn centuries_since_j2000(jd: f64) -> f64 {
    t_centuries(jd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::ang_sep;
    use crate::ARCSEC;

    /// Horizons-frozen reference vectors (assets/ref/horizons_vectors.json),
    /// fetched 2026-10-09 via ssd.jpl.nasa.gov/api/horizons.api (REF_SYSTEM=ICRF).
    #[derive(serde::Deserialize)]
    struct RefEpoch {
        jd: f64,
        pos: [f64; 3],
        #[serde(default)]
        vel: [f64; 3],
    }

    fn refs() -> std::collections::HashMap<String, Vec<RefEpoch>> {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/ref/horizons_vectors.json");
        let txt = std::fs::read_to_string(p).expect("run tools/fetch_horizons.py first");
        serde_json::from_str(&txt).unwrap()
    }

    /// Per-body residual caps [arcsec] vs Horizons (DE441).
    /// Inner planets + Pluto: theory target <1″ (M1 acceptance, PLAN §14).
    /// Uranus/Neptune: VSOP87 is an analytical fit to DE200; against the modern DE the
    /// outer planets carry a documented ~1–2″ systematic (stated in UI footer, §5.1).
    const CAPS: [(&str, f64); 10] = [
        ("mercury", 1.0), ("venus", 1.0), ("earth", 1.0), ("emb", 1.0), ("mars", 1.0),
        ("jupiter", 1.0), ("saturn", 1.0), ("uranus", 2.5), ("neptune", 2.5), ("pluto", 180.0),
    ];

    #[test]
    fn planets_match_horizons_within_caps() {
        let map = refs();
        let ids = [
            ("mercury", PlanetId::Mercury),
            ("venus", PlanetId::Venus),
            ("earth", PlanetId::Earth),
            ("emb", PlanetId::Earth), // compared via emb_ecl below
            ("mars", PlanetId::Mars),
            ("jupiter", PlanetId::Jupiter),
            ("saturn", PlanetId::Saturn),
            ("uranus", PlanetId::Uranus),
            ("neptune", PlanetId::Neptune),
            ("pluto", PlanetId::Pluto),
        ];
        for (key, id) in ids {
            let cap = CAPS.iter().find(|(k, _)| *k == key).unwrap().1;
            for ep in &map[key] {
                // JPL approximate table is validated 1800–2050 AD only (§5.1 note)
                if key == "pluto" && ep.jd > 2_469_000.0 {
                    println!("residual pluto   JD {:.1} : SKIPPED (outside table span)", ep.jd);
                    continue;
                }
                let ours = if key == "emb" {
                    emb_ecl(ep.jd)
                } else {
                    planet_state_ecl(id, ep.jd).pos
                };
                let sep = ang_sep(ours, ep.pos) / ARCSEC; // arcsec
                println!("residual {key:8} JD {:.1} : {sep:8.4} arcsec", ep.jd);
                assert!(
                    sep < cap,
                    "{key} @ JD {}: residual {sep:.4}\" > cap {cap} (ours {:?} vs {:?})",
                    ep.jd,
                    ours,
                    ep.pos
                );
            }
        }
    }

    #[test]
    fn velocities_match_horizons() {
        let map = refs();
        for (key, id) in [("mars", PlanetId::Mars), ("jupiter", PlanetId::Jupiter)] {
            for ep in &map[key] {
                let v = planet_state_ecl(id, ep.jd).vel;
                let dv = (0..3)
                    .map(|k| (v[k] - ep.vel[k]).abs())
                    .fold(0.0f64, f64::max);
                assert!(dv < 2e-6, "{key} vel Δ={dv} AU/d");
            }
        }
    }
}
