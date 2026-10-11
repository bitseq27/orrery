//! ORRERY ephemeris engine (PLAN.md §5).
//!
//! Frames: ICRS / J2000 mean equator & equinox is the master frame (§7).
//! VSOP87A gives heliocentric ecliptic J2000 rectangular coordinates; we rotate by the
//! J2000 mean obliquity to reach ICRS-equivalent (frame bias < 0.1″, documented).

pub mod deep_sky;
pub mod deep_sky_data;
pub mod frames;
pub mod galaxy_data;
pub mod kepler;
pub mod moon;
pub mod moon_data;
pub mod moons;
pub mod planets;
pub mod pop_data;
pub mod population;
pub mod probe_data;
pub mod probes;
pub mod sbdb_data;
pub mod small_bodies;
pub mod time;

pub use kepler::{Elements, State};
pub use time::{jd_to_utc_str, now_jd_tt, JD_J2000};

/// Per-body worst-case angular residual [arcsec] against the bundled Horizons
/// freeze (assets/ref/horizons_vectors.json, fetched 2026-10-09). This is the
/// offline "residual log" source (§10.3); online refresh replaces it when LINK=ON.
pub fn validation_residuals() -> Vec<(String, f64)> {
    use crate::frames::ang_sep;
    use crate::planets::{emb_ecl, planet_state_ecl, PlanetId};
    #[derive(serde::Deserialize)]
    struct Ep { jd: f64, pos: [f64; 3] }
    let txt = include_str!("../../../assets/ref/horizons_vectors.json");
    let map: std::collections::HashMap<String, Vec<Ep>> = serde_json::from_str(txt).unwrap();
    let ids = [
        ("mercury", PlanetId::Mercury), ("venus", PlanetId::Venus), ("earth", PlanetId::Earth),
        ("mars", PlanetId::Mars), ("jupiter", PlanetId::Jupiter), ("saturn", PlanetId::Saturn),
        ("uranus", PlanetId::Uranus), ("neptune", PlanetId::Neptune), ("pluto", PlanetId::Pluto),
    ];
    ids.iter()
        .map(|(k, id)| {
            let worst = map[*k]
                .iter()
                .filter(|ep| !(*k == "pluto" && ep.jd > 2_469_000.0))
                .map(|ep| {
                    let ours = planet_state_ecl(*id, ep.jd).pos;
                    ang_sep(ours, ep.pos) / ARCSEC
                })
                .fold(0.0f64, f64::max);
            (k.to_uppercase(), worst)
        })
        .chain({
            let worst = map["emb"]
                .iter()
                .map(|ep| ang_sep(emb_ecl(ep.jd), ep.pos) / ARCSEC)
                .fold(0.0f64, f64::max);
            vec![("EMB".to_string(), worst)]
        })
        .collect()
}

/// Arc-second → radians.
pub const ARCSEC: f64 = 4.848_136_811_095_36e-6;
/// Astronomical unit in km (IAU 2012 exact).
pub const AU_KM: f64 = 149_597_870.7;
/// Gaussian-derived: AU/day → km/s.
pub const AUPERDAY_KMS: f64 = AU_KM / 86_400.0;
