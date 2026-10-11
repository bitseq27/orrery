//! Spacecraft / exploration probes (session 5): heliocentric states baked from
//! JPL Horizons by tools/fetch_probes.py (ecliptic J2000, AU + AU/day, TDB).
//! Propagation model — honest and stated in the UI:
//!   * BOUND states (energy < 0): Kepler two-body about the Sun (orbiters stay
//!     near their planet's orbit; landers ride the small body's orbit).
//!   * ESCAPING states (Voyagers, Pioneers, New Horizons, Ulysses): straight-line
//!     extrapolation of the last tracked velocity (they are on hyperbolic legs;
//!     solar gravity at 100+ AU changes the track by <1 % over decades).
//! SILENT craft keep their LAST tracked state; the app draws them at the
//! propagated position with the status flagged in telemetry.

use crate::frames::ecl_to_icrs;
use crate::kepler::{elements_from_state, Elements, State};
use crate::planets::{planet_state_ecl, PLANET_IDS, PLANET_NAMES};
use crate::probe_data::{PROBES, PROBE_EPOCH_JD};

#[derive(Clone, Copy, Debug)]
pub struct Probe {
    pub name: &'static str,
    pub agency: &'static str,
    pub launch_year: u32,
    pub target: &'static str,
    /// true = spacecraft still tracked/operational
    pub active: bool,
    pub status_note: &'static str,
    /// epoch of the baked state [JD TDB]
    pub epoch: f64,
    /// heliocentric ecliptic-J2000 state at `epoch`
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    /// bound (elliptic) → Kepler elements; else linear extrapolation
    pub bound: bool,
    /// true when the propagation elements were locked to the parent planet's
    /// velocity (planet orbiters: their osculating HELIOCENTRIC elements swing
    /// wildly around the parent's orbit every revolution — two-body propagation
    /// of the raw state walks off to a=60 AU garbage within years).
    pub parent_locked: bool,
    /// parent planet name when locked ("" otherwise)
    pub parent: &'static str,
    /// baked true velocity [AU/d] — telemetry shows THIS, not the locked one
    pub vel_true: [f64; 3],
    pub el: Elements,
}

/// Gaussian gravitational constant squared = GM_sun [AU³/d²].
const MU_SUN: f64 = 0.017_202_098_95 * 0.017_202_098_95;

fn build() -> Vec<Probe> {
    PROBES
        .iter()
        .map(|(name, agency, yr, target, status, note, epoch, x, y, z, vx, vy, vz)| {
            let pos = [*x, *y, *z];
            let vel = [*vx, *vy, *vz];
            // ---- planet-orbiter detection: probe within 0.05 AU of a planet and
            // moving > ~1.4 km/s relative to it → lock propagation to the PARENT's
            // velocity so the marker rides the planet's orbit for decades instead
            // of walking off on a meaningless osculating heliocentric conic.
            let mut prop_vel = vel;
            let mut parent = "";
            for (id, pname) in PLANET_IDS.iter().zip(PLANET_NAMES.iter()) {
                let ps = planet_state_ecl(*id, *epoch);
                let d = ((pos[0] - ps.pos[0]).powi(2)
                    + (pos[1] - ps.pos[1]).powi(2)
                    + (pos[2] - ps.pos[2]).powi(2))
                .sqrt();
                if d < 0.05 {
                    let dv = ((vel[0] - ps.vel[0]).powi(2)
                        + (vel[1] - ps.vel[1]).powi(2)
                        + (vel[2] - ps.vel[2]).powi(2))
                    .sqrt();
                    if dv > 0.0008 {
                        prop_vel = ps.vel;
                        parent = pname;
                    }
                    break;
                }
            }
            let r = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
            let v2 = prop_vel[0] * prop_vel[0] + prop_vel[1] * prop_vel[1] + prop_vel[2] * prop_vel[2];
            // specific orbital energy: < 0 → bound ellipse
            let bound = v2 / 2.0 - MU_SUN / r.max(1e-9) < 0.0;
            let st_icrs = State { pos: ecl_to_icrs(pos), vel: ecl_to_icrs(prop_vel) };
            let mut el = elements_from_state(&st_icrs, 1.0);
            el.epoch = *epoch; // elements_from_state leaves epoch 0
            Probe {
                name,
                agency,
                launch_year: *yr,
                target,
                active: *status == 0,
                status_note: note,
                epoch: *epoch,
                pos,
                vel: prop_vel,
                vel_true: [*vx, *vy, *vz],
                parent_locked: !parent.is_empty(),
                parent,
                bound,
                el,
            }
        })
        .collect()
}

/// All baked probes (built once, shared).
pub fn probes() -> &'static Vec<Probe> {
    static P: std::sync::OnceLock<Vec<Probe>> = std::sync::OnceLock::new();
    P.get_or_init(build)
}

/// Bake epoch of the probe table.
pub fn probe_epoch() -> f64 {
    PROBE_EPOCH_JD
}

/// Heliocentric ICRS state of a probe at `jd` (propagation model per module docs).
pub fn probe_state(p: &Probe, jd: f64) -> State {
    if p.bound {
        return p.el.state(jd);
    }
    let dt = jd - p.epoch;
    State {
        pos: ecl_to_icrs([
            p.pos[0] + p.vel[0] * dt,
            p.pos[1] + p.vel[1] * dt,
            p.pos[2] + p.vel[2] * dt,
        ]),
        vel: ecl_to_icrs(p.vel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_set_is_populated() {
        let ps = probes();
        assert!(ps.len() >= 30, "expected the major probe fleet, got {}", ps.len());
        // every name unique
        for (i, a) in ps.iter().enumerate() {
            assert!(ps[..i].iter().all(|b| b.name != a.name), "dup {}", a.name);
        }
    }

    #[test]
    fn voyagers_are_escaping_and_far() {
        let ps = probes();
        let v1 = ps.iter().find(|p| p.name == "VOYAGER 1").unwrap();
        assert!(!v1.bound, "Voyager 1 must be on a hyperbolic leg");
        let s = probe_state(v1, PROBE_EPOCH_JD);
        let r = s.r_au();
        assert!((165.0..180.0).contains(&r), "V1 r = {r} AU at bake epoch");
        // linear extrapolation moves ~3.6 AU/yr outward
        let s2 = probe_state(v1, PROBE_EPOCH_JD + 365.25);
        assert!((3.0..4.2).contains(&(s2.r_au() - r)), "V1 radial rate");
    }

    #[test]
    fn orbiters_sit_on_their_planets_orbit() {
        let ps = probes();
        let j = ps.iter().find(|p| p.name == "JUNO").unwrap();
        assert!(j.bound, "Juno must be bound");
        let r = probe_state(j, PROBE_EPOCH_JD).r_au();
        let rj = crate::planets::planet_state(crate::planets::PlanetId::Jupiter, PROBE_EPOCH_JD).r_au();
        assert!((r - rj).abs() < 0.1, "Juno {r} vs Jupiter {rj}");
        // parent-locked: a decade of propagation keeps it near Jupiter's orbit
        assert!(j.parent_locked && j.parent == "JUPITER", "Juno must parent-lock");
        let r2 = probe_state(j, PROBE_EPOCH_JD + 3652.5).r_au();
        assert!((4.9..5.6).contains(&r2), "Juno r+10y = {r2}");
        let mro = ps.iter().find(|p| p.name == "MRO").unwrap();
        assert!(mro.parent_locked && mro.parent == "MARS");
        let rm = probe_state(mro, PROBE_EPOCH_JD + 3652.5).r_au();
        assert!((1.35..1.70).contains(&rm), "MRO r+10y = {rm}");
    }

    #[test]
    fn all_states_finite_over_span() {
        for p in probes().iter() {
            for jd in [PROBE_EPOCH_JD, PROBE_EPOCH_JD - 3652.5, PROBE_EPOCH_JD + 3652.5] {
                let s = probe_state(p, jd);
                assert!(s.pos.iter().all(|v| v.is_finite()), "{} non-finite", p.name);
            }
        }
    }
}
