//! Major moons (R2): mean-circle propagation about the parent planet.
//! Elements baked by tools/fetch_moons.py (Horizons ELEMENTS referred to the
//! J2000 ecliptic at epoch J2000, parent-centred).
//! Accuracy class: arc-minute — visual + timing correct (§5.1 note, ADR-004).

use crate::frames::ecl_to_icrs;
use crate::kepler::State;
use crate::moon_data::MOONS;
use crate::planets::{id_from_name, PlanetId};
use crate::time::JD_J2000;
use crate::AU_KM;

#[derive(Clone, Copy, Debug)]
pub struct MoonDef {
    pub name: &'static str,
    pub parent: PlanetId,
    pub a_km: f64,
    /// sidereal period [d], ALWAYS POSITIVE — retrograde motion is encoded by
    /// `i_deg > 90` (the mean-circle below runs λ forward on a plane whose
    /// ecliptic longitude then sweeps backwards).
    pub period_d: f64,
    pub i_deg: f64,
    pub l0_deg: f64,
}

pub fn moon_defs() -> &'static Vec<MoonDef> {
    static DEFS: std::sync::OnceLock<Vec<MoonDef>> = std::sync::OnceLock::new();
    DEFS.get_or_init(build_defs)
}

fn build_defs() -> Vec<MoonDef> {
    MOONS
        .iter()
        .map(|(name, parent, a, per, i, l0, _mass, _rad)| MoonDef {
            name,
            parent: id_from_name(parent).unwrap(),
            a_km: *a,
            period_d: *per,
            i_deg: *i,
            l0_deg: *l0,
        })
        .collect()
}

/// Parent-centred ecliptic-J2000 offset [AU] at jd.
///
/// The circle is inclined by `i` about the x-axis of the parent frame and run
/// forward in λ = L0 + n·t; for i > 90° the ecliptic longitude ≈ −λ, i.e. the
/// moon moves retrograde — no separate period sign needed (a negative period
/// combined with an i>90° plane double-flipped the direction, which rendered
/// Triton prograde; fixed session 4 together with the fetcher).
pub fn moon_offset_ecl(m: &MoonDef, jd: f64) -> [f64; 3] {
    let n = 360.0 / m.period_d; // deg/day, always forward in λ
    let lam = (m.l0_deg + n * (jd - JD_J2000)).to_radians();
    let r = m.a_km / AU_KM;
    let (si, ci) = (m.i_deg.to_radians().sin(), m.i_deg.to_radians().cos());
    [r * lam.cos(), r * lam.sin() * ci, r * lam.sin() * si]
}

/// Heliocentric ICRS state of a moon at jd.
pub fn moon_state(m: &MoonDef, jd: f64) -> State {
    let ps = crate::planets::planet_state_ecl(m.parent, jd);
    let o = moon_offset_ecl(m, jd);
    let h = 0.02;
    let o0 = moon_offset_ecl(m, jd - h);
    let o1 = moon_offset_ecl(m, jd + h);
    let vo = [
        (o1[0] - o0[0]) / (2.0 * h),
        (o1[1] - o0[1]) / (2.0 * h),
        (o1[2] - o0[2]) / (2.0 * h),
    ];
    State {
        pos: ecl_to_icrs([ps.pos[0] + o[0], ps.pos[1] + o[1], ps.pos[2] + o[2]]),
        vel: ecl_to_icrs([ps.vel[0] + vo[0], ps.vel[1] + vo[1], ps.vel[2] + vo[2]]),
    }
}

/// Published masses [kg] and mean radii [km] for the readout cells, from the
/// baked Horizons OBJ_DATA (GM / Mean Radius); 0 = unpublished → UI shows ---.
pub fn moon_props(name: &str) -> (f64, f64) {
    if let Some((_, _, _, _, _, _, mass, rad)) = MOONS.iter().find(|(n, ..)| *n == name) {
        return (*mass, *rad);
    }
    (0.0, 0.0)
}

/// Sidereal rotation period [d] for the readout: nearly every major moon is
/// tidally locked (rot = orbital period); the two documented exceptions are
/// Hyperion (chaotic, ~0.5 d effective) and Nereid (not locked).
pub fn moon_rot_days(name: &str) -> Option<f64> {
    match name {
        "HYPERION" => Some(0.55), // chaotic rotation — flag in UI
        "NEREID" => None,          // rotation not established
        _ => MOONS
            .iter()
            .find(|(n, ..)| *n == name)
            .map(|(_, _, _, per, ..)| *per),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_populated_and_consistent() {
        assert!(MOONS.len() >= 44, "expected the full major-moon set, got {}", MOONS.len());
        for (name, parent, a, per, i, _l0, mass, rad) in MOONS.iter() {
            assert!(*per > 0.0, "{name}: period must be positive (retrograde via i>90)");
            assert!((0.0..180.0).contains(i), "{name}: bad inclination {i}");
            assert!(*a > 1_000.0, "{name}: a_km {a} implausible");
            assert!(*mass >= 0.0 && *rad >= 0.0, "{name}: negative props");
            assert!(id_from_name(parent).is_some(), "{name}: unknown parent {parent}");
        }
        // retrograde flags land where they belong
        let inc = |n: &str| MOONS.iter().find(|(x, ..)| *x == n).unwrap().4;
        assert!(inc("TRITON") > 90.0, "Triton must be retrograde");
        assert!(inc("CHARON") > 90.0, "Pluto system must be retrograde");
        assert!(inc("TITANIA") > 90.0, "Uranian moons must be retrograde");
        assert!(inc("PHOEBE") > 90.0, "Phoebe must be retrograde");
        assert!(inc("IO") < 90.0 && inc("LUNA") < 90.0, "prograde moons");
    }

    #[test]
    fn galilean_phases_are_jovian() {
        // regression: the old fetcher used Saturn IDs (601..604) for the
        // Galileans, baking Mimas-through-Rhea longitudes into Io..Callisto.
        // The corrected L0(J2000) values must agree with Horizons ELEMENTS.
        let l0 = |n: &str| MOONS.iter().find(|(x, ..)| *x == n).unwrap().5;
        assert!((l0("IO") - 18.171).abs() < 0.5, "IO L0 {}", l0("IO"));
        assert!((l0("EUROPA") - 212.687).abs() < 0.5, "EUROPA L0 {}", l0("EUROPA"));
        assert!((l0("GANYMEDE") - 220.029).abs() < 0.5, "GANYMEDE L0 {}", l0("GANYMEDE"));
        assert!((l0("CALLISTO") - 79.188).abs() < 0.5, "CALLISTO L0 {}", l0("CALLISTO"));
    }

    #[test]
    fn moon_positions_track_parent() {
        let defs = moon_defs();
        let io = defs.iter().find(|m| m.name == "IO").unwrap();
        let jd = 2_461_322.5;
        let ms = moon_state(io, jd);
        let ps = crate::planets::planet_state_ecl(PlanetId::Jupiter, jd);
        let pe = crate::frames::ecl_to_icrs(ps.pos);
        let d = ((ms.pos[0] - pe[0]).powi(2) + (ms.pos[1] - pe[1]).powi(2) + (ms.pos[2] - pe[2]).powi(2)).sqrt();
        let expect = io.a_km / AU_KM;
        assert!((d - expect).abs() < expect * 1e-9, "Io–Jupiter sep {d} vs {expect}");
    }
}
