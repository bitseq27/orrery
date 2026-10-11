//! Static physical-property tables for the SELECTED OBJECT readout (session-4
//! request: temperature, composition, rotation "cycle", age).
//!
//! Sources: NASA planetary fact sheets / IAU working-group values / JPL SBDB
//! (spectral classes, baked per-body by tools/fetch_sbdb.py and mapped in
//! orrery_astro::small_bodies). Live-computed quantities (small-body and
//! exoplanet equilibrium temperature) are marked and derived at runtime —
//! nothing here is invented at display time.

use orrery_astro::planets::PlanetId;

// ---- planets (order = PLANET_IDS: Mercury..Pluto) ---------------------------

/// Mean temperature [K]: surface mean for terrestrials, 1-bar level for giants.
pub const PLANET_MEAN_T_K: [f64; 9] = [
    440.0,  // Mercury (diurnal mean; −173..+427 °C swing)
    737.0,  // Venus (surface)
    288.0,  // Earth (surface)
    210.0,  // Mars (surface mean)
    165.0,  // Jupiter (1 bar)
    134.0,  // Saturn (1 bar)
    76.0,   // Uranus (1 bar)
    72.0,   // Neptune (1 bar)
    44.0,   // Pluto (surface)
];

/// Temperature-provenance note per planet.
pub const PLANET_T_NOTE: [&str; 9] = [
    "SURF MEAN", "SURF", "SURF MEAN", "SURF MEAN",
    "1-BAR", "1-BAR", "1-BAR", "1-BAR", "SURF",
];

/// Bulk/atmospheric composition, short form for the readout.
pub const PLANET_COMP: [&str; 9] = [
    "Fe CORE · SILICATE MANTLE",
    "ROCK · ATM CO2 96% N2 3.5%",
    "ROCK/Fe · ATM N2 78% O2 21%",
    "BASALT/Fe2O3 · ATM CO2 95%",
    "GAS: H2 90% He 10% · ROCK CORE",
    "GAS: H2 96% He 3% · ROCK CORE",
    "ICE GIANT: H2/He · H2O-CH4",
    "ICE GIANT: H2/He · H2O-CH4",
    "N2/CH4/CO ICE · ROCK",
];

/// Sidereal rotation period [d]; negative = retrograde.
pub const PLANET_ROT_D: [f64; 9] = [
    58.646, -243.025, 0.997_27, 1.025_96, 0.413_54, 0.444_01, -0.718_33, 0.671_25, -6.387_23,
];

/// Formation age [Gyr].
pub const PLANET_AGE_GYR: [f64; 9] = [4.60; 9];

// ---- Sun --------------------------------------------------------------------
pub const SUN: &str = "SOL";
pub const SUN_MASS_KG: f64 = 1.9885e30;
pub const SUN_RADIUS_KM: f64 = 695_700.0;
pub const SUN_TEFF_K: f64 = 5772.0;
pub const SUN_COMP: &str = "H 73.4% · He 24.9% · METALS 1.7%";
pub const SUN_ROT_D: f64 = 25.38; // equatorial sidereal
pub const SUN_AGE_GYR: f64 = 4.603;

// ---- moons ------------------------------------------------------------------

/// Measured/estimated mean temperature [K] for moons where published;
/// others fall back to a live blackbody equilibrium value from heliocentric r.
pub fn moon_mean_t_k(name: &str) -> Option<f64> {
    Some(match name {
        "LUNA" => 220.0,
        "PHOBOS" | "DEIMOS" => 233.0,
        "IO" => 130.0,
        "EUROPA" => 102.0,
        "GANYMEDE" => 110.0,
        "CALLISTO" => 134.0,
        "MIMAS" | "TETHYS" => 64.0,
        "ENCELADUS" => 75.0,
        "DIONE" => 74.0,
        "RHEA" => 76.0,
        "TITAN" => 94.0,
        "IAPETUS" => 95.0,
        "HYPERION" => 90.0,
        "PHOEBE" => 80.0,
        "MIRANDA" | "ARIEL" => 60.0,
        "UMBRIEL" => 65.0,
        "TITANIA" | "OBERON" => 70.0,
        "TRITON" => 38.0,
        "PROTEUS" | "LARISSA" => 51.0,
        "NEREID" => 45.0,
        "CHARON" | "STYX" | "NIX" | "KERBEROS" | "HYDRA" => 40.0,
        _ => return None,
    })
}

/// Bulk composition class for moons.
pub fn moon_comp(name: &str) -> &'static str {
    match name {
        "LUNA" => "ANORTHOSITE CRUST · BASALT MARE",
        "PHOBOS" | "DEIMOS" => "CARBONACEOUS? REGOLITH (CAPTURED)",
        "METIS" | "ADRASTEA" | "AMALTHEA" | "THEBE" => "SILICATE ROCK / REGOLITH",
        "IO" => "SILICATE · SULFUR/SO2 FROST",
        "EUROPA" => "H2O ICE SHELL · SILICATE · Fe CORE",
        "GANYMEDE" => "H2O ICE · SILICATE · Fe CORE",
        "CALLISTO" => "H2O ICE / ROCK MIXTURE",
        "HIMALIA" | "ELARA" | "LYSITHEA" | "LEDA" => "ROCK (CAPTURED, PROGRADE)",
        "ANANKE" | "CARME" | "PASIPHAE" | "SINOPE" => "ROCK/ICE (CAPTURED, RETROGRADE)",
        "JANUS" | "EPIMETHEUS" => "H2O ICE / ROCK (POROUS)",
        "MIMAS" | "TETHYS" | "DIONE" | "RHEA" => "H2O ICE · ROCK",
        "ENCELADUS" => "H2O ICE · SUBSURFACE OCEAN",
        "TITAN" => "H2O ICE ROCK · ATM N2 95% CH4",
        "HYPERION" => "H2O ICE/ROCK · SPONGE-LIKE",
        "IAPETUS" => "H2O ICE · DARK LEADING SIDE",
        "PROMETHEUS" | "PANDORA" => "H2O ICE / ROCK (SHEPHERD)",
        "PHOEBE" => "ICE/ROCK · DARK (CAPTURED KBO?)",
        "MIRANDA" | "ARIEL" | "UMBRIEL" | "TITANIA" | "OBERON" => "H2O ICE / ROCK · DARK MATERIAL",
        "LARISSA" | "PROTEUS" => "ICE / DARK ORGANIC MATERIAL",
        "TRITON" => "N2/CH4/CO ICE · ROCK CORE (CAPTURED)",
        "NEREID" => "ICE/ROCK (ECCENTRIC ORBIT)",
        "CHARON" => "H2O ICE / ROCK",
        "STYX" | "NIX" | "KERBEROS" | "HYDRA" => "H2O ICE / ROCK (DEBRIS)",
        _ => "ICE/ROCK (UNK)",
    }
}

/// Formation age [Gyr] for moons.
pub fn moon_age_gyr(name: &str) -> f64 {
    match name {
        "LUNA" => 4.51,  // giant-impact formation
        "CHARON" | "STYX" | "NIX" | "KERBEROS" | "HYDRA" => 4.5, // impact debris
        _ => 4.5,
    }
}

// ---- derived helpers ---------------------------------------------------------

/// Blackbody equilibrium temperature [K] at heliocentric distance r [AU] for
/// Bond albedo A (fast-rotator / isothermal approximation):
/// T = 278.6 K · ((1−A)/r²)^(1/4). Used for airless small bodies & exoplanets
/// (honest label shown next to the value).
pub fn equilibrium_t_k(r_au: f64, albedo: f64) -> f64 {
    278.6 * ((1.0 - albedo) / (r_au * r_au).max(1e-9)).powf(0.25)
}

/// Exoplanet composition class from published radius/mass (Chen & Kipping-style
/// regimes; honest — classification, not measurement).
pub fn exo_composition(radius_rj: Option<f64>, mass_mj: Option<f64>) -> &'static str {
    const R_EARTH_IN_RJ: f64 = 1.0 / 11.209;
    if let Some(r) = radius_rj {
        let re = r / R_EARTH_IN_RJ;
        return if re < 1.6 {
            "ROCKY (TERRAN)"
        } else if re < 3.5 {
            "ROCK/VOLATILES (SUPER-EARTH)"
        } else if re < 6.0 {
            "ROCK CORE · H/He ENVELOPE"
        } else if r < 0.55 {
            "ICE GIANT (H2O/CH4/NH3 · H/He)"
        } else {
            "GAS GIANT (H2/He)"
        };
    }
    if let Some(m) = mass_mj {
        let me = m * 317.8;
        return if me < 2.0 {
            "ROCKY (TERRAN, MASS-ONLY)"
        } else if me < 10.0 {
            "ROCK/VOLATILES (MASS-ONLY)"
        } else if me < 50.0 {
            "ICE GIANT? (MASS-ONLY)"
        } else {
            "GAS GIANT (MASS-ONLY)"
        };
    }
    "UNK (NO R/M PUBLISHED)"
}

/// Planet index → static table lookup (kept here so telemetry stays readable).
pub fn planet_tables(id: PlanetId) -> (f64, &'static str, &'static str, f64, f64, &'static str) {
    use orrery_astro::planets::PLANET_IDS;
    let i = PLANET_IDS.iter().position(|p| *p == id).unwrap();
    (
        PLANET_MEAN_T_K[i],
        PLANET_T_NOTE[i],
        PLANET_COMP[i],
        PLANET_ROT_D[i],
        PLANET_AGE_GYR[i],
        "",
    )
}
