//! Session-6: SPECTRAL ANALYSIS panel data.
//!
//! Honest modelling, not fake curves: every spectrum is a *documented model* —
//! a continuum shape (slope / blackbody) multiplied by Gaussian absorption
//! bands at published band centres (µm), plus narrow emission lines where the
//! object class actually emits (cometary CN/C2/C3, stellar Fraunhofer lines in
//! reflected-light star spectra). Band centres come from the standard
//! visible–NIR identification lists (CH4 0.89/1.0/1.16/1.7/2.2-2.3, H2O ice
//! 1.03/1.25/1.5/2.0, Fe2+/Fe3+ 0.53/0.86/1.0/1.9-2.0, CO2 ice 2.0-2.15,
//! vegetation red edge 0.72, O2-A 0.76, Na D 0.589, Ca II 0.393/0.397, Hα 0.656).
//! The panel labels every band it draws and states the model class, so nothing
//! is presented as measured data. Composition bars are bulk mass-fraction
//! tables (planetary fact-sheet style) per body class.
//!
//! Session 7 adds the fifth graph: the MIR *fingerprint region* curve
//! (4000–400 cm⁻¹, FTIR convention). Dozens of sharp solid-state vibrational
//! bands (silicate Reststrahlen, ice lattice modes, organic C–H stretches,
//! salts, carbonates) are assembled per body from the same published
//! composition tables, giving each body a composition-unique pattern — and a
//! nearest-archetype MATCH score from curve cross-correlation.
//!
//! Session 8 adds the sixth graph: the rotation light curve, Fourier-
//! synthesised from the published rotation period plus documented per-class
//! harmonic amplitudes (double-peaked for elongated rocky bodies).

use crate::telemetry::Telemetry;

/// One absorption band: centre µm, depth 0..1, gaussian width µm, label.
pub type Band = (f32, f32, f32, &'static str);
/// One emission line: centre µm, strength, label.
pub type Line = (f32, f32, &'static str);

pub struct SpecCurve {
    /// sampled reflectance / relative flux, λ = 0.35..2.5 µm
    pub pts: Vec<[f32; 2]>,
    pub bands: Vec<Band>,
    pub lines: Vec<Line>,
    pub title: String,
    pub note: &'static str,
    /// geometric albedo (None = not applicable)
    pub albedo: Option<f32>,
}

pub const LAM_MIN: f32 = 0.35;
pub const LAM_MAX: f32 = 2.50;
const N: usize = 116;

fn gauss(x: f32, c: f32, w: f32) -> f32 {
    let d = (x - c) / w;
    (-d * d).exp()
}

/// Continuum families.
fn continuum(kind: &str, lam: f32) -> f32 {
    match kind {
        // neutral regolith: nearly flat, slight red slope
        "flat" => 0.55 + 0.10 * (lam - 0.55),
        // dark carbonaceous: flat, UV drop-off
        "cflat" => 0.30 + 0.25 * gauss(lam, 0.30, 0.14) * -1.0 + 0.06 * (lam - 0.55),
        // silicate/metal: moderate red slope
        "red" => 0.35 + 0.38 * (lam - 0.35).clamp(0.0, 2.2),
        // very red organics (tholins, D/P-types, Titan)
        "vred" => 0.18 + 0.55 * (lam - 0.35).clamp(0.0, 2.2).powf(1.2),
        // icy: bright, slightly blue
        "ice" => 0.85 - 0.10 * (lam - 0.55),
        // cloud decks: bright cream
        "cloud" => 0.80 - 0.05 * (lam - 0.55),
        // comet dust: steep red continuum
        "dust" => 0.20 + 0.45 * (lam - 0.35).clamp(0.0, 2.2),
        // star: blackbody-ish in this window (normalised)
        "star" => {
            let t = 1.0;
            (1.0 / (lam.powf(5.0) * ((t / lam) * 2.2).exp() - 1.0)).clamp(0.0, 5.0)
        }
        _ => 0.5,
    }
}

fn build_range(cont: &str, bands: &[Band], lines: &[Line], lmin: f32, lmax: f32, n: usize) -> Vec<[f32; 2]> {
    let mut raw: Vec<f32> = (0..n)
        .map(|k| {
            let lam = lmin + (lmax - lmin) * k as f32 / (n as f32 - 1.0);
            let mut f = continuum(cont, lam);
            for (c, d, w, _) in bands {
                f *= 1.0 - d * gauss(lam, *c, *w);
            }
            for (c, s, _) in lines {
                f += s * gauss(lam, *c, 0.012);
            }
            f.max(0.0)
        })
        .collect();
    let mx = raw.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
    raw.iter_mut().for_each(|v| *v /= mx);
    raw.iter()
        .enumerate()
        .map(|(k, v)| [lmin + (lmax - lmin) * k as f32 / (n as f32 - 1.0), *v])
        .collect()
}

fn build(cont: &str, bands: &[Band], lines: &[Line]) -> Vec<[f32; 2]> {
    build_range(cont, bands, lines, LAM_MIN, LAM_MAX, N)
}

/// Planck spectral radiance shape (λ in µm), unnormalised.
fn planck(lam: f32, t_k: f32) -> f32 {
    let t = t_k.max(1.0);
    let x = 14387.77 / (lam.max(1e-3) * t);
    if x > 80.0 {
        return 0.0;
    }
    1.0 / (lam.powf(5.0) * (x.exp() - 1.0))
}

/// Small-body spectral class → (continuum, bands, albedo).
fn asteroid_spec(spec: &str) -> (&'static str, Vec<Band>, f32) {
    match spec.chars().next().unwrap_or('?') {
        'C' | 'B' | 'F' | 'G' => ("cflat", vec![(0.42, 0.25, 0.10, "UV PHYLLOSILICATE"), (0.70, 0.10, 0.06, "PHYLLO 0.7")], 0.06),
        'S' | 'A' | 'R' | 'Q' => ("red", vec![(1.00, 0.35, 0.10, "Fe2+ OLIVINE/PYX 1µm"), (1.95, 0.25, 0.12, "PYX 2µm")], 0.20),
        'V' => ("red", vec![(0.92, 0.45, 0.08, "PYX 0.9µm"), (1.90, 0.40, 0.10, "PYX 1.9µm")], 0.35),
        'M' | 'X' | 'E' => ("flat", vec![(0.90, 0.08, 0.10, "METAL (WEAK)")], 0.12),
        'D' | 'P' | 'T' => ("vred", vec![], 0.04),
        'K' | 'L' | 'O' => ("red", vec![(1.05, 0.25, 0.10, "OLIVINE 1µm")], 0.15),
        _ => ("flat", vec![], 0.10),
    }
}

/// Spectrum model for the selected object.
pub fn spectrum(t: &Telemetry) -> SpecCurve {
    let name = t.name.as_str();
    match t.kind {
        "PROBE" => SpecCurve {
            pts: build("flat", &[], &[]),
            bands: vec![],
            lines: vec![],
            title: format!("{name} — SPACECRAFT"),
            note: "NO NATURAL SPECTRUM — ARTIFICIAL OBJECT",
            albedo: None,
        },
        kind if kind.contains("STAR") => SpecCurve {
            pts: build(
                "star",
                &[
                    (0.393, 0.35, 0.010, "Ca II K"),
                    (0.397, 0.32, 0.010, "Ca II H"),
                    (0.486, 0.22, 0.010, "Hβ"),
                    (0.517, 0.18, 0.010, "Mg b"),
                    (0.589, 0.25, 0.010, "Na D"),
                    (0.656, 0.30, 0.012, "Hα"),
                ],
                &[],
            ),
            bands: vec![
                (0.393, 0.35, 0.010, "Ca II K"),
                (0.397, 0.32, 0.010, "Ca II H"),
                (0.486, 0.22, 0.010, "Hβ"),
                (0.517, 0.18, 0.010, "Mg b"),
                (0.589, 0.25, 0.010, "Na D"),
                (0.656, 0.30, 0.012, "Hα"),
            ],
            lines: vec![],
            title: format!("{name} — PHOTOSPHERE MODEL"),
            note: "PLANCK CONTINUUM + FRAUNHOFER LINES (MODEL)",
            albedo: None,
        },
        "PLANET" | "DWARF PLANET" => planet_spectrum(name),
        "MOON" => moon_spectrum(name),
        "COMET" => {
            let bands: Vec<Band> = vec![(0.42, 0.10, 0.05, "DUST ABSORPTION")];
            let lines: Vec<Line> = vec![
                (0.388, 0.55, "CN 388nm"),
                (0.406, 0.40, "C3 406nm"),
                (0.426, 0.30, "CO+ 426nm"),
                (0.474, 0.35, "C2 474nm"),
                (0.514, 0.45, "C2 514nm"),
                (0.630, 0.25, "[OI] 630nm"),
            ];
            SpecCurve {
                pts: build("dust", &bands, &lines),
                bands,
                lines,
                title: format!("{name} — COMA MODEL"),
                note: "DUST CONTINUUM + GAS EMISSION LINES (MODEL)",
                albedo: Some(0.04),
            }
        }
        "ASTEROID" => {
            // taxonomic letter: SBDB spectral class when published, else a
            // family template chosen from the SBDB orbit class (labelled as such)
            let cls = t.class.to_uppercase();
            let letter = if !t.composition.contains("UNK SPEC") && !t.composition.is_empty() {
                asteroid_letter_from_composition(t)
            } else {
                ""
            };
            let (spec, templ) = if !letter.is_empty() {
                (letter, false)
            } else {
                let l = if cls.contains("MAIN BELT") {
                    "C"
                } else if cls.contains("TROJAN")
                    || cls.contains("TRANS-NEPTUNIAN")
                    || cls.contains("SCATTERED")
                    || cls.contains("HILDA")
                {
                    "D"
                } else if cls.contains("NEA")
                    || cls.contains("APOLLO")
                    || cls.contains("ATEN")
                    || cls.contains("AMOR")
                    || cls.contains("INNER-EARTH")
                {
                    "S"
                } else {
                    asteroid_letter_from_composition(t)
                };
                (l, true)
            };
            let (cont, bands, alb) = asteroid_spec(spec);
            SpecCurve {
                pts: build(cont, &bands, &[]),
                bands,
                lines: vec![],
                title: format!("{name} — {}-TYPE MODEL", spec),
                note: if templ {
                    "ORBIT-FAMILY TEMPLATE (NO PUBLISHED SPECTRAL CLASS)"
                } else {
                    "TAXONOMIC-CLASS TEMPLATE (SBDB SPECTRAL CLASS)"
                },
                albedo: Some(alb),
            }
        }
        "EXOPLANET" => {
            // reflected-light model: host continuum + class bands by size
            let gas = t.radius_km > 0.8 * 69911.0;
            let (cont, mut bands): (&str, Vec<Band>) = if gas {
                (
                    "cloud",
                    vec![
                        (0.589, 0.30, 0.012, "Na D"),
                        (0.770, 0.25, 0.012, "K 0.77"),
                        (0.890, 0.35, 0.03, "CH4 0.89"),
                        (1.160, 0.30, 0.04, "CH4 1.16"),
                        (1.400, 0.40, 0.05, "CH4/H2O 1.4"),
                        (2.200, 0.45, 0.08, "CH4 2.2"),
                    ],
                )
            } else {
                ("flat", vec![(1.00, 0.20, 0.09, "PYX 1µm (MODEL)"), (1.95, 0.15, 0.10, "PYX 2µm (MODEL)")])
            };
            if t.temp_k.unwrap_or(0.0) > 1200.0 {
                bands.push((1.400, 0.50, 0.06, "H2O 1.4 (HOT)"));
            }
            SpecCurve {
                pts: build(cont, &bands, &[]),
                bands,
                lines: vec![],
                title: format!("{name} — REFLECTED-LIGHT MODEL"),
                note: "TWO-BODY CATALOG PLANET — TEMPLATE SPECTRUM (§5.2 CEILING)",
                albedo: Some(if gas { 0.5 } else { 0.3 }),
            }
        }
        _ => SpecCurve {
            pts: build("flat", &[], &[]),
            bands: vec![],
            lines: vec![],
            title: format!("{name} — NO MODEL"),
            note: "---",
            albedo: None,
        },
    }
}

fn asteroid_letter_from_composition(t: &Telemetry) -> &'static str {
    // real SBDB bodies carry their Tholen/SMASS letter in the raw table; the
    // telemetry composition string encodes the class family — recover it
    if t.composition.contains("CARBONACEOUS") {
        "C"
    } else if t.composition.contains("SILICATE") {
        "S"
    } else if t.composition.contains("METALLIC") {
        "M"
    } else if t.composition.contains("BASALTIC") {
        "V"
    } else if t.composition.contains("ORGANIC-RICH") || t.composition.contains("TRANS-NEPTUNIAN") {
        "D"
    } else {
        "C"
    }
}

fn planet_spectrum(name: &str) -> SpecCurve {
    let (cont, bands, alb, note): (&str, Vec<Band>, f32, &'static str) = match name {
        "MERCURY" => (
            "flat",
            vec![(0.90, 0.15, 0.08, "Fe2+ 0.9µm"), (1.90, 0.10, 0.10, "PYX 1.9µm")],
            0.142,
            "REGOLITH REFLECTANCE MODEL",
        ),
        "VENUS" => (
            "cloud",
            vec![(0.36, 0.35, 0.08, "UV ABSORBER"), (1.40, 0.10, 0.05, "H2O 1.4"), (2.30, 0.15, 0.08, "CO2 2.3")],
            0.689,
            "CLOUD-DECK REFLECTANCE MODEL",
        ),
        "EARTH" => (
            "ice",
            vec![
                (0.60, 0.25, 0.06, "H2O/OCEAN"),
                (0.76, 0.12, 0.015, "O2 A-BAND"),
                (0.94, 0.18, 0.03, "H2O 0.94"),
                (1.13, 0.15, 0.03, "H2O 1.13"),
                (1.40, 0.35, 0.04, "H2O 1.4"),
                (1.90, 0.40, 0.05, "H2O 1.9"),
            ],
            0.306,
            "DISK-INTEGRATED MODEL (VEGETATION RED EDGE MARKED)",
        ),
        "MARS" => (
            "red",
            vec![
                (0.53, 0.20, 0.05, "Fe3+ 0.53"),
                (0.86, 0.22, 0.06, "Fe3+ 0.86"),
                (1.00, 0.18, 0.08, "PYX 1µm"),
                (1.95, 0.22, 0.10, "PYX/H2O 2µm"),
                (2.05, 0.15, 0.05, "CO2 ICE 2.0"),
            ],
            0.170,
            "FE-OXIDE REGOLITH REFLECTANCE MODEL",
        ),
        "JUPITER" => (
            "cloud",
            vec![
                (0.890, 0.30, 0.03, "CH4 0.89"),
                (1.000, 0.25, 0.03, "CH4 1.0"),
                (1.160, 0.30, 0.04, "CH4 1.16"),
                (1.400, 0.35, 0.05, "CH4/NH3 1.4"),
                (1.700, 0.30, 0.05, "CH4 1.7"),
                (2.300, 0.40, 0.08, "CH4 2.3"),
            ],
            0.503,
            "CLOUD + CH4 ABSORPTION MODEL",
        ),
        "SATURN" => (
            "cloud",
            vec![
                (0.890, 0.22, 0.03, "CH4 0.89"),
                (1.000, 0.18, 0.03, "CH4 1.0"),
                (1.160, 0.22, 0.04, "CH4 1.16"),
                (1.400, 0.28, 0.05, "CH4/NH3 1.4"),
                (2.300, 0.32, 0.08, "CH4 2.3"),
            ],
            0.342,
            "CLOUD + CH4 ABSORPTION MODEL",
        ),
        "URANUS" => (
            "cloud",
            vec![
                (0.890, 0.45, 0.03, "CH4 0.89"),
                (1.000, 0.40, 0.03, "CH4 1.0"),
                (1.160, 0.45, 0.04, "CH4 1.16"),
                (1.400, 0.50, 0.05, "CH4 1.4"),
                (1.700, 0.45, 0.05, "CH4 1.7"),
                (2.200, 0.60, 0.08, "CH4 2.2"),
            ],
            0.300,
            "DEEP CH4 ICE-GIANT MODEL",
        ),
        "NEPTUNE" => (
            "cloud",
            vec![
                (0.890, 0.50, 0.03, "CH4 0.89"),
                (1.000, 0.45, 0.03, "CH4 1.0"),
                (1.160, 0.50, 0.04, "CH4 1.16"),
                (1.400, 0.55, 0.05, "CH4 1.4"),
                (1.700, 0.50, 0.05, "CH4 1.7"),
                (2.200, 0.65, 0.08, "CH4 2.2"),
            ],
            0.290,
            "DEEP CH4 ICE-GIANT MODEL",
        ),
        _ => (
            "ice",
            vec![
                (0.890, 0.35, 0.03, "CH4 0.89"),
                (1.500, 0.20, 0.05, "H2O 1.5"),
                (1.700, 0.30, 0.05, "CH4 1.7"),
                (2.000, 0.25, 0.06, "H2O 2.0"),
                (2.150, 0.20, 0.05, "N2 2.15"),
                (2.350, 0.30, 0.06, "CH4 2.35"),
            ],
            0.52,
            "N2/CH4 ICE SURFACE MODEL (PLUTO CLASS)",
        ),
    };
    // Earth: mark the vegetation red edge as an emission-like step
    let mut lines: Vec<Line> = vec![];
    if name == "EARTH" {
        lines.push((0.72, 0.18, "RED EDGE 0.72"));
    }
    SpecCurve {
        pts: build(cont, &bands, &lines),
        bands,
        lines,
        title: format!("{name} — REFLECTANCE MODEL"),
        note,
        albedo: Some(alb),
    }
}

fn moon_spectrum(name: &str) -> SpecCurve {
    let icy = matches!(
        name,
        "EUROPA" | "GANYMEDE" | "CALLISTO" | "ENCELADUS" | "TETHYS" | "DIONE" | "RHEA" | "IAPETUS" | "TRITON" | "TITANIA" | "OBERON" | "ARIEL" | "UMBRIEL" | "MIRANDA" | "CHARON"
    );
    let tholin = matches!(name, "TITAN" | "IAPETUS");
    let io = name == "IO";
    let (cont, bands, alb): (&str, Vec<Band>, f32) = if tholin {
        ("vred", vec![(0.42, 0.30, 0.10, "THOLIN UV")], 0.20)
    } else if io {
        ("flat", vec![(0.40, 0.35, 0.08, "S/SO2 0.4"), (0.56, 0.20, 0.06, "S 0.56")], 0.63)
    } else if icy {
        (
            "ice",
            vec![
                (1.03, 0.25, 0.04, "H2O 1.03"),
                (1.25, 0.30, 0.05, "H2O 1.25"),
                (1.50, 0.55, 0.06, "H2O 1.5"),
                (2.00, 0.60, 0.07, "H2O 2.0"),
            ],
            0.60,
        )
    } else {
        ("flat", vec![(0.95, 0.15, 0.08, "PYX 1µm"), (1.90, 0.10, 0.10, "PYX 2µm")], 0.11)
    };
    SpecCurve {
        pts: build(cont, &bands, &[]),
        bands,
        lines: vec![],
        title: format!("{name} — SURFACE MODEL"),
        note: if icy { "WATER-ICE REFLECTANCE MODEL" } else if tholin { "ORGANIC THOLIN SLOPE MODEL" } else if io { "SULFUR/SO2 MODEL" } else { "ROCKY REGOLITH MODEL" },
        albedo: Some(alb),
    }
}

/// Bulk composition (mass fractions, sum ≈ 1): (label, fraction, rgb).
pub fn composition(t: &Telemetry) -> Vec<(&'static str, f32, [u8; 3])> {
    let c = |s: &'static str, f: f32, rgb: [u8; 3]| (s, f, rgb);
    match t.kind {
        "PROBE" => vec![],
        kind if kind.contains("STAR") => vec![
            c("H", 0.735, [255, 120, 120]),
            c("He", 0.249, [255, 220, 120]),
            c("O", 0.008, [120, 200, 255]),
            c("C", 0.003, [160, 160, 160]),
            c("OTHER", 0.005, [120, 120, 120]),
        ],
        "PLANET" | "DWARF PLANET" => match t.name.as_str() {
            "MERCURY" => vec![c("Fe", 0.70, [200, 160, 110]), c("Ni", 0.05, [170, 170, 180]), c("Si", 0.13, [150, 200, 150]), c("O", 0.12, [120, 200, 255])],
            "VENUS" | "EARTH" => vec![
                c("Fe", 0.32, [200, 160, 110]),
                c("O", 0.30, [120, 200, 255]),
                c("Si", 0.15, [150, 200, 150]),
                c("Mg", 0.14, [200, 200, 120]),
                c("Ni", 0.02, [170, 170, 180]),
                c("S", 0.02, [230, 230, 100]),
                c("OTHER", 0.05, [120, 120, 120]),
            ],
            "MARS" => vec![c("O", 0.40, [120, 200, 255]), c("Fe", 0.25, [200, 160, 110]), c("Si", 0.19, [150, 200, 150]), c("Mg", 0.13, [200, 200, 120]), c("S", 0.02, [230, 230, 100]), c("OTHER", 0.01, [120, 120, 120])],
            "JUPITER" => vec![c("H", 0.75, [255, 120, 120]), c("He", 0.24, [255, 220, 120]), c("CH4/NH3/H2O", 0.01, [120, 200, 255])],
            "SATURN" => vec![c("H", 0.96, [255, 120, 120]), c("He", 0.03, [255, 220, 120]), c("ICES", 0.01, [120, 200, 255])],
            "URANUS" | "NEPTUNE" => vec![c("H2O/CH4/NH3 ICE", 0.80, [120, 200, 255]), c("H", 0.15, [255, 120, 120]), c("He", 0.03, [255, 220, 120]), c("ROCK", 0.02, [150, 200, 150])],
            _ => vec![c("ROCK", 0.70, [150, 200, 150]), c("N2/CH4/CO ICE", 0.25, [120, 200, 255]), c("OTHER", 0.05, [120, 120, 120])],
        },
        "MOON" => match t.name.as_str() {
            "LUNA" => vec![c("O", 0.43, [120, 200, 255]), c("Si", 0.21, [150, 200, 150]), c("Mg", 0.19, [200, 200, 120]), c("Fe", 0.10, [200, 160, 110]), c("Ca", 0.03, [220, 180, 220]), c("Al", 0.03, [190, 190, 190]), c("OTHER", 0.01, [120, 120, 120])],
            "IO" => vec![c("S", 0.30, [230, 230, 100]), c("Si", 0.25, [150, 200, 150]), c("O", 0.35, [120, 200, 255]), c("Fe", 0.10, [200, 160, 110])],
            "TITAN" => vec![c("H2O ICE", 0.55, [120, 200, 255]), c("ROCK", 0.35, [150, 200, 150]), c("ORGANICS", 0.10, [200, 120, 80])],
            "EUROPA" | "ENCELADUS" => vec![c("H2O ICE", 0.90, [120, 200, 255]), c("ROCK/SALT", 0.10, [150, 200, 150])],
            "TRITON" | "CHARON" => vec![c("H2O ICE", 0.55, [120, 200, 255]), c("ROCK", 0.40, [150, 200, 150]), c("N2/CH4 ICE", 0.05, [200, 220, 255])],
            _ => vec![c("H2O ICE", 0.50, [120, 200, 255]), c("ROCK", 0.45, [150, 200, 150]), c("OTHER", 0.05, [120, 120, 120])],
        },
        "COMET" => vec![c("H2O ICE", 0.80, [120, 200, 255]), c("CO/CO2", 0.12, [180, 180, 255]), c("DUST/ORGANICS", 0.08, [200, 160, 110])],
        "ASTEROID" => {
            if t.composition.contains("METALLIC") {
                vec![c("Fe", 0.88, [200, 160, 110]), c("Ni", 0.10, [170, 170, 180]), c("OTHER", 0.02, [120, 120, 120])]
            } else if t.composition.contains("CARBONACEOUS") || t.composition.contains("SYNTHETIC · ROCK") {
                vec![c("O", 0.46, [120, 200, 255]), c("Si", 0.18, [150, 200, 150]), c("Mg", 0.14, [200, 200, 120]), c("Fe", 0.12, [200, 160, 110]), c("C", 0.05, [160, 160, 160]), c("S", 0.02, [230, 230, 100]), c("H2O/OH", 0.03, [180, 220, 255])]
            } else if t.composition.contains("ICE") {
                vec![c("H2O ICE", 0.50, [120, 200, 255]), c("ROCK", 0.40, [150, 200, 150]), c("CH4/N2 ICE", 0.10, [200, 220, 255])]
            } else {
                vec![c("O", 0.42, [120, 200, 255]), c("Si", 0.20, [150, 200, 150]), c("Mg", 0.16, [200, 200, 120]), c("Fe", 0.18, [200, 160, 110]), c("Ni", 0.02, [170, 170, 180]), c("S", 0.02, [230, 230, 100])]
            }
        }
        "EXOPLANET" => {
            if t.radius_km > 0.8 * 69911.0 {
                vec![c("H", 0.74, [255, 120, 120]), c("He", 0.24, [255, 220, 120]), c("ICES/METALS", 0.02, [120, 200, 255])]
            } else {
                vec![c("Si", 0.25, [150, 200, 150]), c("O", 0.40, [120, 200, 255]), c("Fe", 0.25, [200, 160, 110]), c("Mg", 0.10, [200, 200, 120])]
            }
        }
        _ => vec![],
    }
}

/// Thermal-infrared emissivity features per body class (µm, depth, width, label).
fn mir_bands(t: &Telemetry) -> (f32, Vec<Band>) {
    let n = t.name.as_str();
    let default_t = t.temp_k.unwrap_or(220.0) as f32;
    let bands: Vec<Band> = match t.kind {
        "PLANET" | "DWARF PLANET" => match n {
            "MARS" => vec![(4.3, 0.35, 0.25, "CO2 4.3"), (15.0, 0.55, 1.6, "CO2 15"), (9.0, 0.20, 0.8, "DUST 9")],
            "VENUS" => vec![(4.3, 0.45, 0.25, "CO2 4.3"), (15.0, 0.60, 1.8, "CO2 15"), (7.3, 0.25, 0.4, "H2SO4 7.3")],
            "EARTH" => vec![(6.3, 0.50, 0.5, "H2O 6.3"), (9.6, 0.35, 0.4, "O3 9.6"), (15.0, 0.55, 1.5, "CO2 15")],
            "JUPITER" | "SATURN" => vec![(3.3, 0.30, 0.2, "CH4 3.3"), (7.7, 0.45, 0.5, "CH4 7.7"), (10.7, 0.30, 0.6, "NH3 10.7")],
            "URANUS" | "NEPTUNE" => vec![(3.3, 0.40, 0.2, "CH4 3.3"), (7.7, 0.55, 0.5, "CH4 7.7")],
            _ => vec![(3.6, 0.30, 0.3, "CH4/N2 ICE"), (4.6, 0.25, 0.3, "CO ICE")],
        },
        "MOON" => vec![(3.1, 0.45, 0.4, "H2O ICE 3.1"), (10.0, 0.35, 0.9, "SILICATE 10"), (18.0, 0.30, 1.4, "SILICATE 18")],
        "COMET" => vec![(3.4, 0.35, 0.3, "ORGANIC 3.4"), (4.6, 0.30, 0.3, "CO 4.6"), (10.0, 0.45, 0.9, "SILICATE 10")],
        "ASTEROID" => vec![(3.4, 0.25, 0.3, "ORGANIC 3.4"), (10.0, 0.45, 0.9, "SILICATE 10"), (18.0, 0.35, 1.4, "SILICATE 18")],
        _ => vec![],
    };
    (default_t, bands)
}

/// UV–VIS electronic features (µm, depth, width, label).
fn uv_bands(t: &Telemetry) -> (&'static str, Vec<Band>, Vec<Line>) {
    let n = t.name.as_str();
    match t.kind {
        kind if kind.contains("STAR") => (
            "star",
            vec![(0.365, 0.45, 0.03, "BALMER JUMP"), (0.28, 0.35, 0.03, "Mg II 0.28")],
            vec![],
        ),
        "PLANET" => match n {
            "EARTH" => ("ice", vec![(0.25, 0.85, 0.05, "O3 HARTLEY"), (0.60, 0.18, 0.06, "O3 CHAPPUIS")], vec![]),
            "VENUS" => ("cloud", vec![(0.32, 0.55, 0.04, "SO2 0.32"), (0.36, 0.45, 0.05, "UV ABSORBER")], vec![]),
            "MARS" => ("red", vec![(0.35, 0.45, 0.06, "DUST UV DROP"), (0.25, 0.30, 0.04, "O3 TRACE")], vec![]),
            _ => ("cloud", vec![(0.28, 0.25, 0.04, "NH3/HAZE UV")], vec![]),
        },
        "MOON" => {
            if n == "IO" {
                ("flat", vec![(0.28, 0.70, 0.05, "SO2 0.28"), (0.32, 0.55, 0.05, "S2 0.32")], vec![])
            } else if n == "TITAN" {
                ("vred", vec![(0.28, 0.75, 0.06, "THOLIN UV")], vec![])
            } else {
                ("ice", vec![(0.28, 0.35, 0.05, "ICE UV DROP")], vec![])
            }
        }
        "COMET" => (
            "dust",
            vec![(0.32, 0.30, 0.05, "DUST UV")],
            vec![(0.336, 0.35, "NH 336nm"), (0.388, 0.55, "CN 388nm"), (0.406, 0.40, "C3 406nm")],
        ),
        "ASTEROID" => {
            if t.composition.contains("CARBONACEOUS") || t.composition.contains("SYNTHETIC") {
                ("cflat", vec![(0.40, 0.40, 0.10, "UV PHYLLOSILICATE")], vec![])
            } else {
                ("red", vec![(0.32, 0.25, 0.06, "UV DROP")], vec![])
            }
        }
        _ => ("flat", vec![], vec![]),
    }
}

// ------------------------------------------------- MIR fingerprint (S7) ----
/// One solid-state fingerprint feature: centre ν̃ [cm⁻¹], depth 0..1,
/// gaussian half-width [cm⁻¹], label. FTIR wavenumber convention.
pub type Fp = (f32, f32, f32, &'static str);

pub const FP_MIN: f32 = 400.0; // cm⁻¹  (25 µm)
pub const FP_MAX: f32 = 4000.0; // cm⁻¹  (2.5 µm)
pub const FP_N: usize = 260;

// Material feature sets. Band centres follow the standard identification
// lists used throughout this module (see header): H2O ice 3150/1645/800/620,
// CO2 2340/667, CO 2138, CH4 3010/1304, NH3 3370/1070, N2 2328,
// SO2 1345/1150, SO4 1120/640, CO3 1430/880/712, Si-O stretch ~1030 and
// bend 550/470, olivine 880/985, pyroxene 920/1050, Mg-OH 3680,
// PAH/aromatic 3050/1610/1300/1160/885, aliphatic C-H 2955/2920/2860,
// O3 1045, H2SO4 1180/900.
const SET_ICE: &[Fp] = &[
    (3150.0, 0.55, 240.0, "H2O OH STR"),
    (1645.0, 0.22, 60.0, "H2O BEND"),
    (800.0, 0.18, 90.0, "H2O LIBR"),
    (620.0, 0.15, 50.0, "H2O LATTICE"),
];
const SET_SIOT: &[Fp] = &[
    (1030.0, 0.38, 90.0, "Si-O STR"),
    (550.0, 0.28, 60.0, "Si-O BEND"),
    (470.0, 0.16, 45.0, "Si-O 470"),
];
const SET_OLIVINE: &[Fp] = &[(880.0, 0.25, 40.0, "OLIV 880"), (985.0, 0.22, 40.0, "OLIV 985")];
const SET_PYX: &[Fp] = &[(920.0, 0.24, 38.0, "PYX 920"), (1050.0, 0.26, 42.0, "PYX 1050")];
const SET_PLAG: &[Fp] = &[(1080.0, 0.30, 110.0, "PLAG Si-O"), (580.0, 0.18, 50.0, "PLAG 580")];
const SET_PHYLLO: &[Fp] = &[
    (3680.0, 0.30, 40.0, "Mg-OH 3680"),
    (3420.0, 0.35, 200.0, "OH/H2O 3420"),
    (1635.0, 0.16, 55.0, "H-OH BEND"),
    (1020.0, 0.45, 110.0, "Si-O PHYLLO"),
    (950.0, 0.18, 45.0, "OH BEND 950"),
];
const SET_ORGANIC: &[Fp] = &[
    (2955.0, 0.25, 40.0, "C-H 2955"),
    (2920.0, 0.28, 40.0, "C-H 2920"),
    (2860.0, 0.18, 35.0, "C-H 2860"),
    (1460.0, 0.14, 40.0, "C-H DEF"),
    (1375.0, 0.13, 35.0, "CH3 1375"),
    (3050.0, 0.10, 40.0, "AROM C-H"),
    (1610.0, 0.12, 50.0, "C=C 6.2UM"),
    (1300.0, 0.12, 60.0, "PAH 7.7UM"),
    (1160.0, 0.10, 45.0, "PAH 8.6UM"),
    (885.0, 0.10, 40.0, "PAH 11.3UM"),
];
const SET_CO2: &[Fp] = &[(2340.0, 0.55, 30.0, "CO2 2340"), (667.0, 0.35, 25.0, "CO2 667")];
const SET_CO: &[Fp] = &[(2138.0, 0.32, 25.0, "CO 2138")];
const SET_CH4: &[Fp] = &[(3010.0, 0.35, 30.0, "CH4 3010"), (1304.0, 0.28, 30.0, "CH4 1304")];
const SET_NH3: &[Fp] = &[(3370.0, 0.28, 60.0, "NH3 3370"), (1070.0, 0.22, 40.0, "NH3 1070")];
const SET_N2: &[Fp] = &[(2328.0, 0.18, 15.0, "N2 2328")];
const SET_SO2: &[Fp] = &[(1345.0, 0.30, 35.0, "SO2 1345"), (1150.0, 0.28, 35.0, "SO2 1150")];
const SET_SULFATE: &[Fp] = &[(1120.0, 0.30, 80.0, "SO4 v3"), (640.0, 0.20, 50.0, "SO4 v4")];
const SET_CARBONATE: &[Fp] = &[
    (1430.0, 0.40, 60.0, "CO3 v3"),
    (880.0, 0.25, 30.0, "CO3 v2"),
    (712.0, 0.15, 25.0, "CO3 v4"),
];
const SET_H2SO4: &[Fp] = &[(1180.0, 0.30, 90.0, "H2SO4 v3"), (900.0, 0.20, 60.0, "H2SO4 v1")];
const SET_O3: &[Fp] = &[(1045.0, 0.25, 40.0, "O3 1045")];

/// The fifth graph's data: fingerprint-region curve + nearest-class match.
pub struct FpCurve {
    /// x = ν̃ [cm⁻¹] ascending 400..4000, y = relative emissivity 0..1
    pub pts: Vec<[f32; 2]>,
    /// all features, sorted deepest first (graph labels the top few)
    pub feats: Vec<Fp>,
    /// short component summary for the title, e.g. "H2O ICE+SILICATE"
    pub material: String,
    /// provenance note (model class), drawn in the plot corner
    pub note: &'static str,
    /// featureless low-emissivity metallic surface
    pub metallic: bool,
    /// nearest archetype label (None = not applicable: stars, probes, unknown)
    pub match_label: Option<&'static str>,
    /// Pearson similarity to that archetype, %
    pub match_pct: Option<u8>,
}

/// Archetype library for the MATCH score — canonical class fingerprints.
struct Arch {
    label: &'static str,
    sets: &'static [&'static [Fp]],
    metallic: bool,
}
const FP_ARCH: &[Arch] = &[
    Arch { label: "S-TYPE OLIVINE/PYX", sets: &[SET_SIOT, SET_OLIVINE, SET_PYX], metallic: false },
    Arch { label: "C-TYPE PHYLLO/CARBON", sets: &[SET_PHYLLO, SET_ORGANIC], metallic: false },
    Arch { label: "V-TYPE BASALTIC PYX", sets: &[SET_SIOT, SET_PYX, SET_PYX], metallic: false },
    Arch { label: "M-TYPE METAL", sets: &[], metallic: true },
    Arch { label: "H2O-ICE WORLD", sets: &[SET_ICE], metallic: false },
    Arch {
        label: "COMETARY MIX",
        sets: &[SET_ICE, SET_CO2, SET_CO, SET_CH4, SET_NH3, SET_ORGANIC, SET_SIOT],
        metallic: false,
    },
    Arch { label: "THOLIN/ORGANIC D-P", sets: &[SET_ORGANIC], metallic: false },
    Arch {
        label: "CO2/SO2 ATM (VENUS/MARS)",
        sets: &[SET_CO2, SET_SIOT, SET_SO2, SET_H2SO4],
        metallic: false,
    },
    Arch { label: "N2/CH4 ICE TNO", sets: &[SET_N2, SET_CH4, SET_CO, SET_ICE], metallic: false },
    Arch { label: "SO2/SULFUR (IO)", sets: &[SET_SO2, SET_SULFATE, SET_SIOT], metallic: false },
    Arch { label: "GAS GIANT CH4/NH3", sets: &[SET_CH4, SET_NH3], metallic: false },
    Arch {
        label: "ICE/ROCK MOON",
        sets: &[SET_ICE, SET_SIOT, SET_OLIVINE, SET_PYX],
        metallic: false,
    },
    Arch {
        label: "OCEAN WORLD H2O/CO2",
        sets: &[SET_ICE, SET_CO2, SET_SIOT, SET_O3],
        metallic: false,
    },
    Arch {
        label: "PLAGIOCLASE ROCK (LUNA)",
        sets: &[SET_PLAG, SET_SIOT, SET_PYX],
        metallic: false,
    },
];

/// Normalised emissivity samples for a set of materials on the FP grid.
fn raw_fp(sets: &[&[Fp]], metallic: bool) -> Vec<f32> {
    let base = if metallic { 0.18 } else { 0.95 };
    let mut v: Vec<f32> = (0..FP_N)
        .map(|k| {
            let nu = FP_MIN + (FP_MAX - FP_MIN) * k as f32 / (FP_N as f32 - 1.0);
            let mut f = base;
            for s in sets {
                for (c, d, w, _) in s.iter() {
                    let g = ((nu - c) / w.max(1.0)).powi(2);
                    f *= 1.0 - d * (-g).exp();
                }
            }
            f.max(0.0)
        })
        .collect();
    let mx = v.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    v.iter_mut().for_each(|x| *x /= mx);
    v
}

/// Archetype curves, computed once and cached (full_spectra runs per frame).
fn arch_curves() -> &'static [Vec<f32>] {
    use std::sync::OnceLock;
    static C: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    C.get_or_init(|| FP_ARCH.iter().map(|a| raw_fp(a.sets, a.metallic)).collect())
}

fn curve_flat(v: &[f32]) -> bool {
    let (mn, mx) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    mx - mn < 1e-6
}

fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let ma = a[..n].iter().sum::<f32>() / n as f32;
    let mb = b[..n].iter().sum::<f32>() / n as f32;
    let (mut sxy, mut sxx, mut syy) = (0f32, 0f32, 0f32);
    for i in 0..n {
        let x = a[i] - ma;
        let y = b[i] - mb;
        sxy += x * y;
        sxx += x * x;
        syy += y * y;
    }
    if sxx < 1e-12 || syy < 1e-12 {
        return 0.0;
    }
    sxy / (sxx.sqrt() * syy.sqrt())
}

/// Nearest archetype by Pearson correlation of the fingerprint curves.
/// Flat (featureless) curves only "match" the flat metal archetype.
fn fp_match(ys: &[f32], metallic: bool) -> (Option<&'static str>, Option<u8>) {
    let body_flat = curve_flat(ys);
    let mut best: Option<(&'static str, f32)> = None;
    for (a, ac) in FP_ARCH.iter().zip(arch_curves().iter()) {
        let r = if body_flat && curve_flat(ac) {
            1.0
        } else if body_flat {
            0.0
        } else {
            pearson(ys, ac)
        };
        let r = if a.metallic == metallic { r } else { r * 0.5 };
        if best.map_or(true, |(_, b)| r > b) {
            best = Some((a.label, r));
        }
    }
    match best {
        Some((l, r)) => (Some(l), Some((r.clamp(0.0, 1.0) * 100.0).round() as u8)),
        None => (None, None),
    }
}

/// Assemble one body's fingerprint curve from its material sets.
fn fp_build(sets: &[&[Fp]], comps: &[&str], metallic: bool, note: &'static str) -> FpCurve {
    let ys = raw_fp(sets, metallic);
    let pts: Vec<[f32; 2]> = ys
        .iter()
        .enumerate()
        .map(|(k, v)| [FP_MIN + (FP_MAX - FP_MIN) * k as f32 / (FP_N as f32 - 1.0), *v])
        .collect();
    let mut feats: Vec<Fp> = sets.iter().flat_map(|s| s.iter().copied()).collect();
    feats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let (match_label, match_pct) = fp_match(&ys, metallic);
    FpCurve {
        pts,
        feats,
        material: if comps.is_empty() { "NONE".into() } else { comps.join("+") },
        note,
        metallic,
        match_label,
        match_pct,
    }
}

/// Not-applicable fingerprint (stars, probes, unknown): flat curve, no match.
fn fp_na(note: &'static str) -> FpCurve {
    let mut fp = fp_build(&[], &[], false, note);
    fp.match_label = None;
    fp.match_pct = None;
    fp
}

/// Asteroid taxonomic letter for the fingerprint, mirroring `spectrum()`:
/// SBDB/composition-derived letter when published, else orbit-class family.
fn fp_ast_letter(t: &Telemetry) -> &'static str {
    if !t.composition.contains("UNK SPEC") && !t.composition.is_empty() {
        return asteroid_letter_from_composition(t);
    }
    let cls = t.class.to_uppercase();
    if cls.contains("MAIN BELT") {
        "C"
    } else if cls.contains("TROJAN")
        || cls.contains("TRANS-NEPTUNIAN")
        || cls.contains("SCATTERED")
        || cls.contains("HILDA")
    {
        "D"
    } else if cls.contains("NEA")
        || cls.contains("APOLLO")
        || cls.contains("ATEN")
        || cls.contains("AMOR")
        || cls.contains("INNER-EARTH")
    {
        "S"
    } else {
        "C"
    }
}

/// Fingerprint-region model for the selected object (the fifth graph).
pub fn fingerprint(t: &Telemetry) -> FpCurve {
    match t.kind {
        "PROBE" => fp_na("ARTIFICIAL OBJECT - NO FINGERPRINT"),
        kind if kind.contains("STAR") => fp_na("GAS PHOTOSPHERE - NO SOLID-STATE FINGERPRINT"),
        "PLANET" | "DWARF PLANET" => match t.name.as_str() {
            "MERCURY" => fp_build(
                &[SET_SIOT, SET_OLIVINE, SET_PYX, SET_PLAG],
                &["SILICATE", "PLAG"],
                false,
                "REGOLITH SILICATE EMISSIVITY MODEL",
            ),
            "VENUS" => fp_build(
                &[SET_CO2, SET_H2SO4, SET_SO2],
                &["CO2 ATM", "H2SO4 CLOUD", "SO2"],
                false,
                "ATMOSPHERE/CLOUD GAS MODEL",
            ),
            "EARTH" => fp_build(
                &[SET_ICE, SET_CO2, SET_O3, SET_SIOT],
                &["H2O", "CO2", "O3", "SILICATE"],
                false,
                "OCEAN+ATM+CRUST MODEL",
            ),
            "MARS" => fp_build(
                &[SET_CO2, SET_SIOT, SET_OLIVINE, SET_SULFATE],
                &["CO2 ATM", "DUST", "SO4"],
                false,
                "ATM+REGOLITH MODEL",
            ),
            "JUPITER" | "SATURN" => {
                fp_build(&[SET_CH4, SET_NH3], &["CH4", "NH3"], false, "CLOUD-TOP GAS MODEL")
            }
            "URANUS" | "NEPTUNE" => fp_build(
                &[SET_CH4, SET_CH4, SET_NH3],
                &["CH4 (DEEP)", "NH3"],
                false,
                "CH4-RICH ATMOSPHERE MODEL",
            ),
            "CERES" => fp_build(
                &[SET_CARBONATE, SET_PHYLLO, SET_ICE],
                &["CARBONATE", "PHYLLO", "H2O ICE"],
                false,
                "DAWN-BASED CERES MIX MODEL",
            ),
            "PLUTO" | "ERIS" | "MAKEMAKE" | "HAUMEA" => fp_build(
                &[SET_N2, SET_CH4, SET_CO, SET_ICE],
                &["N2", "CH4", "CO", "H2O ICE"],
                false,
                "TNO SURFACE ICE MIX MODEL",
            ),
            _ => fp_build(
                &[SET_SIOT, SET_OLIVINE, SET_PYX, SET_ICE],
                &["SILICATE", "H2O?"],
                false,
                "ROCKY DEFAULT MODEL",
            ),
        },
        "EXOPLANET" => {
            let c = t.composition.to_uppercase();
            if c.contains("ICE GIANT") {
                fp_build(&[SET_CH4, SET_CH4, SET_NH3], &["CH4", "NH3"], false, "ICE-GIANT ATMOSPHERE MODEL")
            } else if c.contains("GAS") || c.contains("H/HE") || c.contains("ENVELOPE") {
                fp_build(&[SET_CH4, SET_NH3], &["CH4", "NH3"], false, "GAS-GIANT CLOUD-TOP MODEL")
            } else if c.contains("VOLATILE") {
                fp_build(
                    &[SET_SIOT, SET_OLIVINE, SET_PYX, SET_ICE],
                    &["SILICATE", "H2O"],
                    false,
                    "ROCK+VOLATILES MODEL",
                )
            } else {
                fp_build(&[SET_SIOT, SET_OLIVINE, SET_PYX], &["SILICATE"], false, "ROCKY SURFACE MODEL")
            }
        }
        "MOON" => {
            let n = t.name.as_str();
            let c = t.composition.to_uppercase();
            if n == "IO" {
                fp_build(
                    &[SET_SIOT, SET_PYX, SET_SO2, SET_SULFATE],
                    &["SILICATE", "SO2 FROST"],
                    false,
                    "IO SURFACE MODEL",
                )
            } else if n == "EUROPA" {
                fp_build(
                    &[SET_ICE, SET_SULFATE, SET_SIOT],
                    &["H2O ICE", "SALTS/SO4", "ROCK"],
                    false,
                    "EUROPA ICE+SALT MODEL",
                )
            } else if n == "TRITON" {
                fp_build(
                    &[SET_N2, SET_CH4, SET_CO, SET_ICE],
                    &["N2", "CH4", "CO", "H2O ICE"],
                    false,
                    "TRITON ICE MIX MODEL",
                )
            } else if n == "TITAN" {
                fp_build(&[SET_ICE, SET_ORGANIC], &["H2O ICE", "THOLIN"], false, "TITAN THOLIN/ICE MODEL")
            } else {
                // keyword-driven mix from the composition table (same
                // substring conventions as the other graph builders).
                // NB: whole-token checks only — "BASALT" contains "SALT",
                // so the salt keyword must be the plural "SALTS".
                let ice = c.contains("H2O") || c.contains("ICE");
                let ch4 = c.contains("CH4") || c.contains("N2");
                let rock = c.contains("SILICATE") || c.contains("ROCK") || c.contains("REGOLITH");
                let plag = c.contains("ANORTHOSITE") || c.contains("BASALT");
                let basalt = c.contains("BASALT");
                let carb = c.contains("CARBONACEOUS") || c.contains("CHONDRITIC");
                let org = c.contains("DARK") || c.contains("ORGANIC") || c.contains("THOLIN");
                let sulf = c.contains("SULF") || c.contains("SO2");
                let salts = c.contains("SALTS");
                let mut sets: Vec<&[Fp]> = vec![];
                let mut comps: Vec<&str> = vec![];
                if ice {
                    sets.push(SET_ICE);
                    comps.push("H2O ICE");
                }
                if ch4 {
                    sets.push(SET_CH4);
                    comps.push("CH4/N2");
                }
                if rock || plag {
                    sets.push(SET_SIOT);
                    comps.push("SILICATE");
                }
                if rock {
                    sets.push(SET_OLIVINE);
                    sets.push(SET_PYX);
                }
                if plag {
                    sets.push(SET_PLAG);
                    comps.push("PLAG");
                }
                if basalt {
                    sets.push(SET_PYX); // doubled: deeper pyroxene dips
                }
                if carb {
                    sets.push(SET_PHYLLO);
                    comps.push("CARBONACEOUS");
                }
                if org {
                    sets.push(SET_ORGANIC);
                    comps.push("ORGANICS");
                }
                if sulf {
                    sets.push(SET_SO2);
                    comps.push("SO2");
                }
                if salts {
                    sets.push(SET_SULFATE);
                    comps.push("SALTS");
                }
                if sets.is_empty() {
                    fp_na("NO COMPOSITION DATA - FEATURELESS MODEL")
                } else {
                    fp_build(&sets, &comps, false, "SURFACE MIX FROM COMPOSITION TABLE")
                }
            }
        }
        "COMET" => fp_build(
            &[SET_ICE, SET_CO2, SET_CO, SET_CH4, SET_NH3, SET_ORGANIC, SET_SIOT, SET_OLIVINE],
            &["H2O ICE", "CO2", "CO", "CH4", "NH3", "ORGANIC", "DUST"],
            false,
            "NUCLEUS ICE/DUST MIX MODEL",
        ),
        "ASTEROID" => match fp_ast_letter(t) {
            "S" => fp_build(&[SET_SIOT, SET_OLIVINE, SET_PYX], &["OLIVINE", "PYX"], false, "S-TYPE SILICATE MODEL"),
            "V" => fp_build(&[SET_SIOT, SET_PYX, SET_PYX], &["LOW-CA PYX"], false, "V-TYPE BASALTIC MODEL"),
            "M" => fp_build(&[], &[], true, "M-TYPE METAL - FEATURELESS (LOW EMISSIVITY)"),
            "D" => fp_build(&[SET_ORGANIC], &["THOLIN/ORGANIC"], false, "D/P-TYPE ORGANIC MODEL"),
            "K" => fp_build(&[SET_SIOT, SET_OLIVINE, SET_PHYLLO], &["SILICATE", "PHYLLO"], false, "K/L/O CHONDRITIC MODEL"),
            _ => fp_build(
                &[SET_PHYLLO, SET_ORGANIC, SET_CARBONATE],
                &["PHYLLO", "ORGANIC", "CO3"],
                false,
                "C-TYPE CARBONACEOUS MODEL",
            ),
        },
        _ => fp_na("NO COMPOSITION DATA - FEATURELESS MODEL"),
    }
}

// ------------------------------------------- rotation light curve (S8) ----
/// Sixth graph: rotation light curve — the brightness wobble of a spinning
/// body, Fourier-synthesised from its published rotation period plus
/// documented per-class harmonic amplitudes:
/// Δm(φ) = Σₖ Aₖ·sin(2πkφ + ψₖ). Elongated rocky bodies are double-peaked
/// (2nd harmonic dominant, two maxima per rotation); synchronous satellites
/// barely wobble; comets wobble through coma jets. Like every curve in this
/// module it is a *documented model*, labelled as such in the graph.
pub struct LcCurve {
    /// x = rotation phase 0..1, y = Δmag (the renderer flips: brighter up)
    pub pts: Vec<[f32; 2]>,
    pub period_h: Option<f32>,
    pub retro: bool,
    /// (harmonic k, amplitude [mag])
    pub amps: Vec<(u8, f32)>,
    /// peak-to-peak amplitude [mag]
    pub pk_pk: f32,
    pub note: &'static str,
}

pub const LC_N: usize = 128;
/// Fixed harmonic phase offsets — documented modelling assumption: the
/// longitude of the shape/albedo pattern is unknown, so one phase set is
/// used for every body (the *amplitudes* carry the per-class information).
const LC_PSI: [f32; 4] = [0.0, 1.1, 2.3, 3.7];

fn lc_build(period_h: Option<f32>, retro: bool, amps: &[f32], note: &'static str) -> LcCurve {
    let mut pts: Vec<[f32; 2]> = Vec::with_capacity(LC_N);
    let (mut mn, mut mx) = (f32::MAX, f32::MIN);
    for k in 0..LC_N {
        let ph = k as f32 / (LC_N as f32 - 1.0);
        let mut dm = 0.0;
        for (i, av) in amps.iter().enumerate() {
            let kk = i as f32 + 1.0;
            dm += av * (std::f32::consts::TAU * kk * ph + LC_PSI[i]).sin();
        }
        mn = mn.min(dm);
        mx = mx.max(dm);
        pts.push([ph, dm]);
    }
    let amps = amps.iter().enumerate().map(|(i, v)| (i as u8 + 1, *v)).collect();
    LcCurve { pts, period_h, retro, amps, pk_pk: (mx - mn).max(0.0), note }
}

fn lc_na(note: &'static str) -> LcCurve {
    lc_build(None, false, &[], note)
}

/// Rotation light-curve model for the selected object (the sixth graph).
pub fn light_curve(t: &Telemetry) -> LcCurve {
    let Some(pd) = t.rot_d.filter(|d| d.abs() > 1e-9) else {
        return lc_na(match t.kind {
            "PROBE" => "ARTIFICIAL OBJECT - NO LIGHT CURVE",
            _ => "ROTATION PERIOD UNKNOWN - NO LIGHT CURVE",
        });
    };
    let period_h = Some(pd.abs() as f32 * 24.0);
    let retro = pd < 0.0;
    match t.kind {
        "PROBE" => lc_na("ARTIFICIAL OBJECT - NO LIGHT CURVE"),
        kind if kind.contains("STAR") => {
            lc_build(period_h, retro, &[0.001, 0.002, 0.0005], "SPOT MODULATION MODEL (MMAG)")
        }
        "ASTEROID" => {
            // elongated rock: double-peaked; NEA/PHA spinners tend extreme
            let a2 = if t.pha || t.neo {
                0.35
            } else if t.class.contains("TROJAN") || t.class.contains("TRANS-NEPTUNIAN") {
                0.15
            } else {
                0.22
            };
            lc_build(period_h, retro, &[0.35 * a2, a2, 0.18 * a2, 0.08 * a2],
                "ELONGATED-SHAPE MODEL (2ND HARMONIC DOMINANT)")
        }
        "COMET" => lc_build(period_h, retro, &[0.12, 0.22, 0.06], "COMA JET MODEL (NUCLEUS SPIN + JETS)"),
        "MOON" => {
            let a2 = if t.rot_note == "CHAOTIC" {
                0.20
            } else if t.composition.contains("CAPTURED") || t.composition.contains("DARK") || t.composition.contains("SPONGE") {
                0.12
            } else if t.name == "IO" {
                0.05
            } else {
                0.02
            };
            lc_build(period_h, retro, &[0.3 * a2, a2, 0.15 * a2], "SYNCHRONOUS ALBEDO-PATCH MODEL")
        }
        "PLANET" | "DWARF PLANET" | "EXOPLANET" => {
            if t.name == "PLUTO" {
                lc_build(period_h, retro, &[0.3 * 0.06, 0.06, 0.12 * 0.06], "SURFACE-ALBEDO PATCH MODEL")
            } else {
                lc_build(period_h, retro, &[0.3 * 0.01, 0.01, 0.12 * 0.01], "CLOUD-BAND MODEL (MMAG)")
            }
        }
        _ => lc_na("NO ROTATION PERIOD - NO LIGHT CURVE"),
    }
}

/// The six-graph set (sessions 6–8): VIS-NIR reflectance, UV-VIS features,
/// MIR thermal emission, a log-log reflected+thermal SED, the MIR
/// fingerprint-region curve with its nearest-class MATCH score, and the
/// Fourier rotation light curve.
pub struct FullSpec {
    pub vis: SpecCurve,
    pub uv: SpecCurve,
    pub mir_pts: Vec<[f32; 2]>,
    pub mir_bands: Vec<Band>,
    pub mir_t: f32,
    pub sed_pts: Vec<[f32; 2]>,
    pub sed_note: &'static str,
    /// session 7: MIR fingerprint-region curve + nearest-class match
    pub fp: FpCurve,
    /// session 8: Fourier rotation light curve
    pub lc: LcCurve,
}

pub fn full_spectra(t: &Telemetry) -> FullSpec {
    let vis = spectrum(t);
    let fp = fingerprint(t);
    let lc = light_curve(t);
    let (uv_cont, uv_b, uv_l) = uv_bands(t);
    let uv = SpecCurve {
        pts: build_range(uv_cont, &uv_b, &uv_l, 0.20, 0.90, 90),
        bands: uv_b.clone(),
        lines: uv_l.clone(),
        title: format!("{} — UV-VIS", t.name),
        note: "ELECTRONIC BANDS (MODEL)",
        albedo: vis.albedo,
    };
    let (t_k, mir_b) = mir_bands(t);
    // thermal emission: Planck(T) × emissivity dips, λ = 2..50 µm
    let n = 140;
    let mut raw: Vec<f32> = (0..n)
        .map(|k| {
            let lam = 2.0 + 48.0 * k as f32 / (n as f32 - 1.0);
            let mut f = planck(lam, t_k);
            for (c, d, w, _) in &mir_b {
                f *= 1.0 - d * gauss(lam, *c, *w);
            }
            f
        })
        .collect();
    let mx = raw.iter().cloned().fold(0.0f32, f32::max).max(1e-30);
    raw.iter_mut().for_each(|v| *v /= mx);
    let mir_pts: Vec<[f32; 2]> = raw
        .iter()
        .enumerate()
        .map(|(k, v)| [2.0 + 48.0 * k as f32 / (n as f32 - 1.0), *v])
        .collect();
    // SED: reflected sunlight (albedo × 5772 K Planck) + body thermal, log-log.
    // Components scaled to comparable peaks for display (stated in the panel).
    let alb = vis.albedo.unwrap_or(0.1).max(0.01);
    let sn = 64;
    let mut sed: Vec<[f32; 2]> = Vec::with_capacity(sn);
    let mut peak_ref = 1e-30f32;
    let mut peak_th = 1e-30f32;
    let mut ref_c: Vec<f32> = Vec::with_capacity(sn);
    let mut th_c: Vec<f32> = Vec::with_capacity(sn);
    for k in 0..sn {
        let ll = -0.5 + 2.5 * k as f32 / (sn as f32 - 1.0); // log10 µm: 0.316..316 µm
        let lam = 10f32.powf(ll);
        let r = alb * planck(lam, 5772.0);
        let th = planck(lam, t_k.max(1.0));
        ref_c.push(r);
        th_c.push(th);
        peak_ref = peak_ref.max(r);
        peak_th = peak_th.max(th);
    }
    for k in 0..sn {
        let ll = -0.5 + 2.5 * k as f32 / (sn as f32 - 1.0);
        let f = ref_c[k] / peak_ref + 0.85 * th_c[k] / peak_th;
        sed.push([ll, f.max(1e-4)]);
    }
    let smx = sed.iter().map(|q| q[1]).fold(0.0f32, f32::max).max(1e-30);
    sed.iter_mut().for_each(|q| q[1] /= smx);
    FullSpec {
        vis,
        uv,
        mir_pts,
        mir_bands: mir_b,
        mir_t: t_k,
        sed_pts: sed,
        sed_note: "REFLECTED (SOL 5772 K × ALBEDO) + THERMAL, PEAKS SCALED FOR DISPLAY",
        fp,
        lc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tel(name: &str, kind: &'static str, comp: &'static str) -> Telemetry {
        Telemetry {
            a_err: None,
            e_err: None,
            name: name.to_string(),
            kind,
            mass_kg: 0.0,
            radius_km: 0.0,
            color: [0, 0, 0],
            dist_star_au: 1.0,
            vel_kms: 0.0,
            dist_geo_au: 0.0,
            dist_parent_au: 0.0,
            phase_deg: 0.0,
            el: orrery_astro::Elements {
                a: 1.0, e: 0.0, i: 0.0, omega: 0.0, w: 0.0, l0: 0.0, epoch: 0.0, n: 0.0, mu_scale: 1.0, rates: [0.0; 6],
            },
            pos_icrs: [0.0; 3],
            vel_icrs: [0.0; 3],
            period_d: 365.25,
            temp_k: None,
            temp_note: "",
            composition: comp,
            rot_d: None,
            rot_note: "",
            age_gyr: None,
            pha: false,
            neo: false,
            class: String::new(),
            agency: "",
            launched: 0,
            mission: "",
            status: "",
        }
    }

    #[test]
    fn curves_are_normalised_and_finite() {
        for (n, k, c) in [
            ("MARS", "PLANET", ""), ("EARTH", "PLANET", ""), ("JUPITER", "PLANET", ""),
            ("SOL", "STAR (G2V)", ""), ("HALLEY", "COMET", ""), ("CERES", "ASTEROID", "CARBONACEOUS ROCK (C/B)"),
            ("VESTA", "ASTEROID", "BASALTIC (V — DIFFERENTIATED)"), ("EUROPA", "MOON", ""), ("TITAN", "MOON", ""),
            ("VOYAGER 1", "PROBE", ""), ("P/1999-J6", "COMET", ""),
            ("2001-XB254", "ASTEROID", "ROCK/REGOLITH (UNK SPEC)"),
        ] {
            let s = spectrum(&tel(n, k, c));
            assert_eq!(s.pts.len(), N);
            let mx = s.pts.iter().map(|p| p[1]).fold(0.0f32, f32::max);
            assert!((mx - 1.0).abs() < 1e-4, "{n} max {mx}");
            assert!(s.pts.iter().all(|p| p[1].is_finite() && p[1] >= 0.0), "{n} finite");
            for (cc, _, _, _) in &s.bands {
                assert!((LAM_MIN..=LAM_MAX).contains(cc), "{n} band {cc}");
            }
        }
    }

    #[test]
    fn bands_dip_and_lines_spike() {
        let s = spectrum(&tel("MARS", "PLANET", ""));
        // at the 0.86 µm Fe3+ band the curve must sit below its neighbourhood
        let at = |lam: f32| s.pts.iter().min_by_key(|p| ((p[0] - lam).abs() * 1000.0) as u32).unwrap()[1];
        assert!(at(0.86) < at(0.70) && at(0.86) < at(1.20), "Mars 0.86 band missing");
        let c = spectrum(&tel("HALLEY", "COMET", ""));
        let atc = |lam: f32| c.pts.iter().min_by_key(|p| ((p[0] - lam).abs() * 1000.0) as u32).unwrap()[1];
        assert!(atc(0.514) > atc(0.45), "C2 emission missing");
    }

    #[test]
    fn four_graph_set_is_consistent() {
        for (n, k, c) in [
            ("MARS", "PLANET", ""), ("EARTH", "PLANET", ""), ("JUPITER", "PLANET", ""),
            ("CERES", "ASTEROID", "CARBONACEOUS ROCK (C/B)"), ("HALLEY", "COMET", ""), ("EUROPA", "MOON", ""),
        ] {
            let f = full_spectra(&tel(n, k, c));
            assert_eq!(f.vis.pts.len(), N);
            assert_eq!(f.uv.pts.len(), 90);
            assert_eq!(f.mir_pts.len(), 140);
            assert_eq!(f.sed_pts.len(), 64);
            for pts in [&f.vis.pts, &f.uv.pts, &f.mir_pts] {
                let mx = pts.iter().map(|q| q[1]).fold(0.0f32, f32::max);
                assert!((mx - 1.0).abs() < 1e-4, "{n}");
                assert!(pts.iter().all(|q| q[0].is_finite() && q[1].is_finite()), "{n}");
            }
            // Wien check: the MIR thermal peak must sit near 2898/T µm
            let peak = f.mir_pts.iter().cloned().max_by(|a, b| a[1].partial_cmp(&b[1]).unwrap()).unwrap();
            let wien = 2898.0 / f.mir_t.max(1.0);
            assert!((peak[0] - wien).abs() < 3.5, "{n} mir peak {} vs wien {}", peak[0], wien);
        }
        // probes carry no natural spectrum
        let f = full_spectra(&tel("VOYAGER 1", "PROBE", ""));
        assert!(f.mir_bands.is_empty());
    }

    #[test]
    fn fingerprint_region_graph_is_consistent() {
        for (n, k, c) in [
            ("MARS", "PLANET", ""), ("EARTH", "PLANET", ""), ("JUPITER", "PLANET", ""),
            ("PLUTO", "DWARF PLANET", ""), ("CERES", "ASTEROID", "CARBONACEOUS ROCK (C/B)"),
            ("VESTA", "ASTEROID", "BASALTIC (V — DIFFERENTIATED)"), ("HALLEY", "COMET", ""),
            ("EUROPA", "MOON", ""), ("TITAN", "MOON", ""), ("IO", "MOON", ""),
            ("2001-XB254", "ASTEROID", "ROCK/REGOLITH (UNK SPEC)"),
        ] {
            let fp = fingerprint(&tel(n, k, c));
            assert_eq!(fp.pts.len(), FP_N, "{n}");
            assert!(fp.pts.iter().all(|q| q[1].is_finite() && q[1] >= 0.0 && q[1] <= 1.0001), "{n} range");
            assert!((fp.pts.first().unwrap()[0] - FP_MIN).abs() < 1e-3, "{n} x0");
            assert!((fp.pts.last().unwrap()[0] - FP_MAX).abs() < 1e-3, "{n} x1");
            let mx = fp.pts.iter().map(|q| q[1]).fold(0.0f32, f32::max);
            assert!((mx - 1.0).abs() < 1e-4, "{n} normalised");
            for (cc, _, _, _) in &fp.feats {
                assert!((FP_MIN..=FP_MAX).contains(cc), "{n} feature {cc}");
            }
            if let Some(p) = fp.match_pct {
                assert!(p <= 100 && fp.match_label.is_some(), "{n} match");
            }
        }
        // sharp features produce real dips: an icy moon must fall well below its baseline
        let fp = fingerprint(&tel("RHEA", "MOON", "H2O ICE · ROCK"));
        let mn = fp.pts.iter().map(|q| q[1]).fold(1.0f32, f32::min);
        assert!(mn < 0.7, "no fingerprint dips (min {mn})");
        // an ice+rock moon correlates strongly with an ice-bearing archetype
        assert!(fp.match_pct.unwrap_or(0) >= 55, "weak ice match {:?}", fp.match_label);
        // metallic asteroid matches the featureless metal archetype exactly
        let fp = fingerprint(&tel("PSYCHE", "ASTEROID", "METALLIC (M)"));
        assert_eq!(fp.match_label, Some("M-TYPE METAL"));
        assert_eq!(fp.match_pct, Some(100));
        // stars and probes carry no solid-state fingerprint
        for (n, k) in [("SOL", "STAR (G2V)"), ("VOYAGER 1", "PROBE")] {
            let fp = fingerprint(&tel(n, k, ""));
            assert!(fp.match_label.is_none() && fp.match_pct.is_none(), "{n}");
        }
        // different compositions → different fingerprints (the uniqueness property)
        let a = fingerprint(&tel("EUROPA", "MOON", "H2O ICE SHELL · SILICATE · Fe CORE"));
        let b = fingerprint(&tel("IO", "MOON", "SILICATE · SULFUR/SO2 FROST"));
        let d = a.pts.iter().zip(&b.pts).map(|(p, q)| (p[1] - q[1]).powi(2)).sum::<f32>().sqrt();
        assert!(d > 0.5, "ice moon vs sulfur moon fingerprints too similar ({d})");
    }

    #[test]
    fn light_curve_fourier_consistent() {
        let with_rot = |rot: f64, t: Telemetry| Telemetry { rot_d: Some(rot), ..t };
        // elongated asteroid: 2nd-harmonic dominant → two minima per rotation,
        // half a phase apart; period converts days → hours
        let lc = light_curve(&with_rot(0.2226, tel("VESTA", "ASTEROID", "BASALTIC (V)")));
        assert_eq!(lc.pts.len(), LC_N);
        let ph = lc.period_h.unwrap();
        assert!((5.3..5.4).contains(&ph), "period {ph} h");
        let mut mins = vec![];
        for i in 1..LC_N - 1 {
            let (a, b, c) = (lc.pts[i - 1][1], lc.pts[i][1], lc.pts[i + 1][1]);
            if b < a && b <= c {
                mins.push(lc.pts[i][0]);
            }
        }
        assert_eq!(mins.len(), 2, "double-peaked expected, mins {mins:?}");
        assert!((mins[1] - mins[0] - 0.5).abs() < 0.08, "minima half a phase apart");
        assert!((0.4..0.9).contains(&lc.pk_pk), "pk-pk {}", lc.pk_pk);
        // periodic: phase 0 and phase 1 agree
        assert!((lc.pts[0][1] - lc.pts[LC_N - 1][1]).abs() < 1e-3);
        // synchronous icy satellite: barely any wobble
        let lc = light_curve(&with_rot(3.55, tel("EUROPA", "MOON", "H2O ICE SHELL · SILICATE")));
        assert!(lc.pk_pk < 0.08 && lc.period_h.is_some());
        // no published rotation period → not applicable, flat
        let lc = light_curve(&tel("SOL", "STAR (G2V)", ""));
        assert!(lc.period_h.is_none() && lc.pk_pk == 0.0 && lc.pts.len() == LC_N);
        // retrograde spin keeps a positive period and flags RETRO
        let lc = light_curve(&with_rot(-0.5, tel("X", "ASTEROID", "SILICATE ROCK (S/Q)")));
        assert!(lc.retro && lc.period_h.unwrap() > 0.0);
    }

    #[test]
    fn compositions_sum_to_one() {
        for (n, k, c) in [
            ("EARTH", "PLANET", ""), ("JUPITER", "PLANET", ""), ("SOL", "STAR (G2V)", ""),
            ("LUNA", "MOON", ""), ("HALLEY", "COMET", ""), ("CERES", "ASTEROID", "CARBONACEOUS ROCK (C/B)"),
        ] {
            let sum: f32 = composition(&tel(n, k, c)).iter().map(|(_, f, _)| f).sum();
            assert!((sum - 1.0).abs() < 0.02, "{n} sum {sum}");
        }
        assert!(composition(&tel("VOYAGER 1", "PROBE", "")).is_empty());
    }
}
