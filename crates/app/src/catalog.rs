//! Exoplanet catalog (R5, §6.2) — bundled at compile time from assets/catalog.json
//! (fetched from the NASA Exoplanet Archive TAP by tools/fetch_exocat.py).

use orrery_astro::kepler::Elements;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Catalog {
    pub systems: Vec<System>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct System {
    pub idx: usize,
    pub host: Host,
    pub planets: Vec<Planet>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Host {
    pub name: String,
    pub spectral: String,
    pub mass_msun: Option<f64>,
    pub radius_rsun: Option<f64>,
    pub teff_k: Option<f64>,
    pub ra_deg: Option<f64>,
    pub dec_deg: Option<f64>,
    pub dist_ly: Option<f64>,
    pub vmag: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Planet {
    pub name: String,
    pub a_au: f64,
    pub a_err: Option<f64>,
    pub e: f64,
    pub e_err: Option<f64>,
    pub i_deg: Option<f64>,
    pub omega_deg: Option<f64>,
    pub w_deg: Option<f64>,
    pub m0_deg: Option<f64>,
    pub period_d: f64,
    pub mass_mj: Option<f64>,
    pub radius_rj: Option<f64>,
}

impl Catalog {
    /// Bundled asset (PLAN §13: self-contained, offline).
    pub fn load() -> Catalog {
        let txt = include_str!("../../../assets/catalog.json");
        serde_json::from_str(txt).expect("catalog.json malformed — rerun tools/fetch_exocat.py")
    }

    /// Two-body elements for a catalog planet (§5.2): Kepler about the host star,
    /// Ω unpublished for RV orbits → 0 (stated in UI provenance).
    pub fn elements(&self, sys: usize, pl: usize) -> Elements {
        let s = &self.systems[sys];
        let p = &s.planets[pl];
        let w = p.w_deg.unwrap_or(0.0);
        let om = p.omega_deg.unwrap_or(0.0);
        let m0 = p.m0_deg.unwrap_or(0.0);
        Elements {
            a: p.a_au,
            e: p.e,
            i: p.i_deg.unwrap_or(0.0),
            omega: om,
            w,
            l0: (m0 + w + om) % 360.0,
            epoch: orrery_astro::time::JD_J2000,
            n: 360.0 / p.period_d,
            mu_scale: s.host.mass_msun.unwrap_or(1.0),
            rates: [0.0; 6],
        }
    }
}
