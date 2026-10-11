//! Asteroids & comets (R3, R4): JPL SBDB osculating elements + Kepler two-body.
//! Elements and physical properties baked by tools/fetch_sbdb.py. Comet tails are
//! visual (§6.1); the `pha` flag (JPL "potentially hazardous": Earth-MOID
//! ≤ 0.05 AU and H ≤ 22) marks near-Earth collision-risk objects red in the scene.

use crate::kepler::{Elements, State};
use crate::sbdb_data::{SBDB, SBDB_PROPS};

#[derive(Clone, Copy, Debug)]
pub struct SmallBody {
    pub name: &'static str,
    pub kind: Kind,
    pub el: Elements,
    pub q_au: f64,
    /// JPL potentially-hazardous flag → red marker (near-Earth collision risk).
    pub pha: bool,
    /// JPL near-Earth-object flag (q < 1.3 AU).
    pub neo: bool,
    /// absolute magnitude [mag] (0 = unpublished)
    pub h_mag: f64,
    /// mean diameter [km] (0 = unpublished)
    pub diam_km: f64,
    /// rotation period [h] (0 = unpublished)
    pub rot_h: f64,
    /// Tholen/SMASS spectral class ("" = unpublished)
    pub spec: &'static str,
    /// SBDB orbit-class code (MBA, APO, ATE, AMO, TNT, JTV, JFC, …)
    pub class: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Asteroid,
    Comet,
}

pub fn small_bodies() -> &'static Vec<SmallBody> {
    static BODIES: std::sync::OnceLock<Vec<SmallBody>> = std::sync::OnceLock::new();
    BODIES.get_or_init(build_bodies)
}

fn build_bodies() -> Vec<SmallBody> {
    SBDB.iter()
        .map(|(name, kind, epoch, a, e, i, om, w, ma, per, q)| {
            let (pha, neo, h, diam, rot, spec, class) = SBDB_PROPS
                .iter()
                .find(|(n, ..)| n == name)
                .map(|(_, pha, neo, h, d, r, s, c)| (*pha == 1, *neo == 1, *h, *d, *r, *s, *c))
                .unwrap_or((false, false, 0.0, 0.0, 0.0, "", ""));
            SmallBody {
                name,
                kind: if *kind == 0 { Kind::Asteroid } else { Kind::Comet },
                el: Elements {
                    a: *a,
                    e: *e,
                    i: *i,
                    omega: *om,
                    w: *w,
                    l0: (ma + om + w) % 360.0,
                    epoch: *epoch,
                    n: if *per > 0.0 { 360.0 / per } else { 0.0 },
                    mu_scale: 1.0,
                    rates: [0.0; 6],
                },
                q_au: *q,
                pha,
                neo,
                h_mag: h,
                diam_km: diam,
                rot_h: rot,
                spec,
                class,
            }
        })
        .collect()
}

impl SmallBody {
    pub fn state(&self, jd: f64) -> State {
        self.el.state(jd)
    }

    /// Visual outgassing activity 0..1 (∝ 1/r² inside the water-ice line region).
    pub fn activity(&self, r_au: f64) -> f32 {
        if self.kind != Kind::Comet {
            return 0.0;
        }
        let a = 2.25 / (r_au * r_au).max(0.04); // 1.0 at r = 1.5 AU
        a.clamp(0.0, 1.0) as f32
    }

    /// Published-ish masses [kg] & radii [km] for readouts (0 = unknown → UI shows ---).
    /// Mass is only measured for the biggest bodies; radius comes from the SBDB
    /// diameter where available.
    pub fn props(&self) -> (f64, f64) {
        let mass = match self.name {
            "CERES" => 9.393e20,
            "PALLAS" => 2.11e20,
            "VESTA" => 2.59e20,
            "HYGIEA" => 8.74e19,
            "INTERAMNIA" => 3.90e19,
            "DAVIDA" => 3.84e19,
            "PSYCHE" => 2.72e19,
            "LUTETIA" => 1.70e18,
            "MATHILDE" => 1.03e17,
            "JUNO" => 2.67e19,
            "IRIS" => 1.35e19,
            "FLORA" => 8.00e18,
            "EROS" => 6.687e15,
            "APOPHIS" => 6.1e10,
            "BENNU" => 7.329e10,
            "RYUGU" => 4.50e11,
            "ITOKAWA" => 3.51e10,
            "TOUTATIS" => 5.0e13,
            "ICARUS" => 1.4e14,
            "PHAETHON" => 1.3e14,
            "ATEN" => 5.5e12,
            "APOLLO" => 1.8e14,
            "CRUITHNE" => 1.3e14,
            "1950-DA" => 4.1e12,
            "GEOGRAPHOS" => 4.0e13,
            "HECTOR" => 1.4e19,
            "PATROCLUS" => 1.67e18,
            "ACHILLES" => 9.6e18,
            "QUAOAR" => 1.2e21,
            "ORCUS" => 6.3e20,
            "MAKEMAKE" => 3.1e21,
            "HAUMEA" => 4.006e21,
            "ERIS" => 1.6466e22,
            "VARUNA" => 3.7e20,
            "SEDNA" => 1.0e21,
            "HALLEY" => 2.2e14,
            "ENCKE" => 1.1e13,
            "TEMPEL-1" => 7.5e13,
            "HALE-BOPP" => 1.3e16,
            "CHURYUMOV" => 9.98e12,
            "BORRELLY" => 2.0e11,
            "WILD-2" => 2.3e12,
            "SWIFT-TUTTLE" => 2.7e14,
            "GIACOBINI-ZINNER" => 1.8e12,
            _ => 0.0,
        };
        let rad = if self.diam_km > 0.0 {
            self.diam_km / 2.0
        } else {
            match self.name {
                "CERES" => 469.7,
                "PALLAS" => 272.0,
                "VESTA" => 262.7,
                "EROS" => 8.4,
                "HALLEY" => 5.5,
                "ENCKE" => 2.4,
                "TEMPEL-1" => 3.0,
                "HALE-BOPP" => 30.0,
                _ => 0.0,
            }
        };
        (mass, rad)
    }

    /// Short composition text for the readout, from the SBDB spectral class /
    /// orbit class (honest: "UNK" when JPL publishes nothing).
    pub fn composition(&self) -> &'static str {
        if self.kind == Kind::Comet {
            return "ICE/DUST (COMETARY)";
        }
        let c = self.class.to_ascii_uppercase();
        if c == "TNO" || c == "CTB" || c == "SDO" {
            return "ICE/ROCK (TRANS-NEPTUNIAN)";
        }
        if c == "TJN" {
            return match self.spec.chars().next().unwrap_or('D') {
                'D' | 'P' | 'C' => "DARK ORGANIC-RICH (TROJAN D/P)",
                _ => "ROCK/ORGANICS (JUPITER TROJAN)",
            };
        }
        match self.spec.chars().next().unwrap_or('?') {
            'C' | 'B' | 'F' | 'G' => "CARBONACEOUS ROCK (C/B)",
            'S' | 'A' | 'R' | 'Q' => "SILICATE ROCK (S/Q)",
            'M' | 'X' | 'E' => "METALLIC Fe-Ni (M/X)",
            'V' => "BASALTIC (V — DIFFERENTIATED)",
            'D' | 'P' | 'T' => "DARK ORGANIC-RICH (D/P/T)",
            'K' | 'L' | 'O' => "SILICATE/CHONDRITIC (K/L/O)",
            _ => "ROCK/REGOLITH (UNK SPEC)",
        }
    }

    /// Orbit-class label for the readout. SBDB codes arrive in mixed case
    /// ("JFc", "ETc") — compare case-insensitively.
    pub fn class_label(&self) -> &'static str {
        let c = self.class.to_ascii_uppercase();
        match c.as_str() {
            "MBA" => "MAIN BELT",
            "APO" => "APOLLO NEA",
            "ATE" => "ATEN NEA",
            "AMO" => "AMOR NEA",
            "IEO" => "INNER-EARTH OBJECT",
            "TJN" => "JUPITER TROJAN",
            "TNO" => "TRANS-NEPTUNIAN",
            "CTB" | "CEN" => "CENTAUR",
            "SDO" => "SCATTERED-DISK OBJECT",
            "JFC" => "JUPITER-FAMILY COMET",
            "ETC" => "ENCKE-TYPE COMET",
            "HTC" => "HALLEY-TYPE COMET",
            "CTC" | "CCC" => "CHIRON-TYPE COMET",
            "JEC" => "QUASI-HILDA COMET",
            "AEC" => "ACTIVE ASTEROID (MBC)",
            _ => {
                if self.kind == Kind::Comet {
                    "PERIODIC COMET"
                } else {
                    "ASTEROID"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn props_cover_elements() {
        assert!(!SBDB.is_empty());
        assert_eq!(SBDB.len(), SBDB_PROPS.len());
        for (name, kind, _ep, a, e, _i, _om, _w, _ma, per, q) in SBDB.iter() {
            assert!(*e < 1.0, "{name}: hyperbolic orbits are outside the 2-body model");
            assert!(*a > 0.0 && *q > 0.0 && *per > 0.0, "{name}: non-positive elements");
            assert!(SBDB_PROPS.iter().any(|(n, ..)| n == name), "{name} missing props");
            let _ = kind;
        }
    }

    #[test]
    fn phas_are_flagged_and_near_earth() {
        let bodies = small_bodies();
        let apophis = bodies.iter().find(|b| b.name == "APOPHIS").expect("Apophis baked");
        assert!(apophis.pha && apophis.neo, "Apophis must be flagged PHA+NEO");
        // every PHA must genuinely cross near Earth's orbit (q < 1.35 AU or
        // a known hazardous comet like 109P/Swift–Tuttle)
        for b in bodies.iter().filter(|b| b.pha) {
            assert!(
                b.q_au < 1.35 || b.name == "SWIFT-TUTTLE",
                "{} flagged PHA with q = {}",
                b.name,
                b.q_au
            );
        }
        assert!(bodies.iter().filter(|b| b.pha).count() >= 8, "expected the well-known PHA set");
    }

    #[test]
    fn more_asteroids_than_before() {
        // session-4 request: "show more asteroids" — the baked set must exceed
        // the original 8 bodies substantially
        assert!(SBDB.len() >= 40, "only {} small bodies baked", SBDB.len());
    }
}
