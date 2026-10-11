//! Deep-sky + galaxy background catalogs (session 5): 100 blurred deep-sky
//! objects (pulsars, quasars, nebulae, clusters, dwarf galaxies, stars, degenerate
//! remnants, black holes) and up to 100 galaxies drawn as blue spirals.
//! Data: tools/fetch_deepsky.py (SIMBAD coordinates + published distances).
//! The renderer treats them as a SKY SPHERE: only direction matters (parallax
//! is negligible at ≥ 100 ly), so they rotate with the camera but never pan/zoom.

use crate::frames::icrs_to_ecl;
use crate::deep_sky_data::DEEP_SKY;
use crate::galaxy_data::GALAXIES;

/// Unit direction (ICRS equatorial J2000) from RA/Dec in degrees.
pub fn dir_icrs(ra_deg: f64, dec_deg: f64) -> [f64; 3] {
    let ra = ra_deg.to_radians();
    let dec = dec_deg.to_radians();
    let cd = dec.cos();
    [cd * ra.cos(), cd * ra.sin(), dec.sin()]
}

/// Unit direction in the ecliptic-J2000 frame (scene master frame).
pub fn dir_ecl(ra_deg: f64, dec_deg: f64) -> [f64; 3] {
    icrs_to_ecl(dir_icrs(ra_deg, dec_deg))
}

pub fn deep_sky() -> &'static [(&'static str, &'static str, f64, f64, f64)] {
    &DEEP_SKY
}

pub fn galaxies() -> &'static [(&'static str, &'static str, f64, f64, f64, f64)] {
    &GALAXIES
}

/// Human distance string (Mly table values span 4e-6 .. 1e4 Mly).
pub fn dist_str(mly: f64) -> String {
    if mly <= 0.0 {
        "---".into()
    } else if mly < 0.001 {
        format!("{:.1} LY", mly * 1e6)
    } else if mly < 1.0 {
        format!("{:.0} KLY", mly * 1e3)
    } else if mly < 1000.0 {
        format!("{:.2} MLY", mly)
    } else {
        format!("{:.2} BLY", mly / 1e3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_within_caps_and_valid() {
        assert!(deep_sky().len() <= 100 && deep_sky().len() >= 90);
        assert!(galaxies().len() <= 100 && galaxies().len() >= 90);
        for (n, _c, ra, dec, d) in deep_sky() {
            assert!((0.0..360.0).contains(ra), "{n} ra {ra}");
            assert!((-90.0..=90.0).contains(dec), "{n} dec {dec}");
            assert!(d.is_finite() && *d > 0.0, "{n} dist");
        }
        for (n, m, ra, dec, d, size) in galaxies() {
            assert!(matches!(*m, "S" | "E" | "I"), "{n} morph {m}");
            assert!((0.0..360.0).contains(ra) && (-90.0..=90.0).contains(dec), "{n} coords");
            assert!(d.is_finite() && *d > 0.0 && size.is_finite() && *size > 0.0, "{n} props");
        }
        // spot checks against the literature
        let find = |q: &str| deep_sky().iter().find(|(n, ..)| *n == q).unwrap();
        let crab = find("PSR B0531+21");
        assert!((crab.2 - 83.63).abs() < 0.1 && (crab.3 - 22.01).abs() < 0.1, "Crab pulsar coords");
        let sgra = find("SAGITTARIUS A*");
        assert!((sgra.2 - 266.42).abs() < 0.15 && (sgra.3 + 29.01).abs() < 0.15, "Sgr A* coords");
        let m31 = galaxies().iter().find(|(n, ..)| n.contains("ANDROMEDA")).unwrap();
        assert!((m31.2 - 10.68).abs() < 0.05 && (m31.3 - 41.27).abs() < 0.05, "M31 coords");
    }

    #[test]
    fn directions_are_unit() {
        for (_n, _c, ra, dec, _d) in deep_sky() {
            let v = dir_icrs(*ra, *dec);
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((l - 1.0).abs() < 1e-12);
        }
    }
}
