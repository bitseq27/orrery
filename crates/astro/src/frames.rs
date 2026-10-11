//! Frame conversions. VSOP87A is heliocentric ecliptic rectangular, equinox J2000.
//! We rotate to the J2000 mean equator (= ICRS to < 0.1″, bias documented in UI footer).

/// J2000 mean obliquity of the ecliptic (IAU 1976/2006): 84381.448″.
pub const EPS0_RAD: f64 = 0.409_092_804_222_328_96; // 23.4392911° in radians

/// Rotate ecliptic J2000 rectangular → equatorial J2000 (ICRS-equivalent).
#[inline]
pub fn ecl_to_icrs(p: [f64; 3]) -> [f64; 3] {
    let (se, ce) = (EPS0_RAD.sin(), EPS0_RAD.cos());
    [p[0], ce * p[1] - se * p[2], se * p[1] + ce * p[2]]
}

/// Inverse: equatorial J2000 (ICRS) → ecliptic J2000.
#[inline]
pub fn icrs_to_ecl(p: [f64; 3]) -> [f64; 3] {
    let (se, ce) = (EPS0_RAD.sin(), EPS0_RAD.cos());
    [p[0], ce * p[1] + se * p[2], -se * p[1] + ce * p[2]]
}

/// Angle between two direction vectors in radians (numerically safe for tiny angles).
pub fn ang_sep(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    cross.iter().map(|c| c * c).sum::<f64>().sqrt().atan2(dot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obliquity_value() {
        assert!((EPS0_RAD.to_degrees() - 23.439_291_1).abs() < 1e-7);
    }

    #[test]
    fn rotation_is_proper() {
        let v = ecl_to_icrs([1.0, 0.0, 0.0]);
        assert!((v[0] - 1.0).abs() < 1e-12 && v[1].abs() < 1e-12 && v[2].abs() < 1e-12);
    }
}
