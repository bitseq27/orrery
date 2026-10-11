//! Lunar theory — truncated ELP-style series (Meeus ch. 47 principal terms).
//! Accuracy class: ~10″ (well inside the §5.1 "arc-minute" budget for moons and more
//! than enough for the Earth-barycentre correction, whose lever arm is ×0.01215).

use crate::time::t_centuries;

/// Geocentric ecliptic (mean equinox of date) position of the Moon [AU].
pub fn moon_geocentric_ecl(jd_tt: f64) -> [f64; 3] {
    let t = t_centuries(jd_tt);
    let d = std::f64::consts::PI / 180.0;
    let lp = (218.316_447_7 + 481_267.881_234_21 * t) * d; // Moon mean longitude
    let dd = (297.850_192_1 + 445_267.111_403_4 * t) * d; // mean elongation
    let m = (357.529_109_2 + 35_999.050_290_9 * t) * d; // Sun mean anomaly
    let mp = (134.963_396_4 + 477_198.867_505_5 * t) * d; // Moon mean anomaly
    let f = (93.272_095_0 + 483_202.017_523_3 * t) * d; // mean latitude argument

    let e = 1.0 - 0.002_516 * t - 0.000_007_4 * t * t;

    // (D, M, M', F, amplitude[0.000001°])
    const LON: [(i32, i32, i32, i32, f64); 24] = [
        (0, 0, 1, 0, 6_288_774.0),
        (2, 0, -1, 0, 1_274_027.0),
        (2, 0, 0, 0, 658_314.0),
        (0, 0, 2, 0, 213_618.0),
        (0, 0, 0, 1, -185_116.0),
        (0, 0, 0, 2, -114_332.0),
        (2, 0, -2, 0, 58_793.0),
        (2, 0, -1, -1, 57_066.0),
        (2, 0, 1, 0, 53_322.0),
        (2, 0, 0, -1, 45_758.0),
        (0, 0, 1, -1, -40_923.0),
        (1, 0, 0, 0, -34_720.0),
        (0, 0, 1, 1, -30_383.0),
        (2, 0, 1, -1, 15_327.0),
        (0, 0, 2, -1, -12_528.0),
        (0, 0, 2, 1, 10_980.0),
        (4, 0, -1, 0, 10_675.0),
        (0, 0, 3, 0, 10_034.0),
        (4, 0, -2, 0, 8_548.0),
        (2, 0, -1, 1, -7_888.0),
        (2, 0, 2, 0, -6_766.0),
        (1, 0, 1, 0, -5_163.0),
        (2, 0, -2, -1, 4_940.0),
        (0, 0, 4, 0, 4_160.0),
    ];
    const LAT: [(i32, i32, i32, i32, f64); 14] = [
        (0, 0, 0, 1, 5_128_122.0),
        (0, 0, 1, 1, 280_602.0),
        (0, 0, 1, -1, 277_693.0),
        (2, 0, 0, -1, 173_237.0),
        (2, 0, -1, 1, 55_413.0),
        (2, 0, -1, -1, 46_271.0),
        (2, 0, 0, 1, 32_573.0),
        (0, 0, 2, 1, 17_198.0),
        (2, 0, 1, -1, 9_206.0),
        (0, 0, 2, -1, 8_822.0),
        (2, 0, 1, 1, 8_216.0),
        (2, 0, -2, -1, 4_324.0),
        (2, 0, 2, -1, 2_200.0), // ≈ (2,0,2,-1) principal
        (0, 0, 1, 2, -2_000.0), // ≈ principal
    ];
    // distance terms [km], cos
    const RAD: [(i32, i32, i32, i32, f64); 16] = [
        (0, 0, 1, 0, -20_905_355.0),
        (2, 0, -1, 0, -3_699_111.0),
        (2, 0, 0, 0, -2_955_968.0),
        (0, 0, 2, 0, -569_925.0),
        (0, 0, 0, 1, 48_888.0),
        (0, 0, 0, 2, -3_149.0),
        (2, 0, -2, 0, 246_158.0),
        (2, 0, -1, -1, -152_138.0),
        (2, 0, 1, 0, -170_733.0),
        (2, 0, 0, -1, -204_586.0),
        (0, 0, 1, -1, -129_620.0),
        (1, 0, 0, 0, 108_743.0),
        (0, 0, 1, 1, 104_755.0),
        (2, 0, 1, -1, 10_321.0),
        (0, 0, 2, -1, 79_661.0),
        (2, 0, 0, 1, -34_782.0),
    ];

    let arg = |c: (i32, i32, i32, i32, f64)| {
        (c.0 as f64) * dd + (c.1 as f64) * m + (c.2 as f64) * mp + (c.3 as f64) * f
    };
    let ec = |c: (i32, i32, i32, i32, f64)| match c.1.abs() {
        1 => e,
        2 => e * e,
        _ => 1.0,
    };

    let mut slon = 0.0;
    for c in LON {
        slon += c.4 * ec(c) * arg(c).sin();
    }
    let mut slat = 0.0;
    for c in LAT {
        slat += c.4 * ec(c) * arg(c).sin();
    }
    let mut sr = 0.0;
    for c in RAD {
        sr += c.4 * ec(c) * arg(c).cos();
    }

    let lon = lp + slon * 1e-6 * d;
    let lat = slat * 1e-6 * d;
    let r_km = 385_000.56 + sr / 1000.0;
    let r_au = r_km / crate::AU_KM;

    [
        r_au * lat.cos() * lon.cos(),
        r_au * lat.cos() * lon.sin(),
        r_au * lat.sin(),
    ]
}

/// Moon/(Earth+Moon) mass ratio — lever arm of the EMB correction.
pub const MOON_MASS_RATIO: f64 = 0.012_150_585_249_141_1; // 1/81.3005…

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_distance_sane() {
        // distance must stay within 356 400 – 406 700 km over a year
        let jd0 = crate::time::JD_J2000;
        for k in 0..40 {
            let p = moon_geocentric_ecl(jd0 + k as f64 * 9.0);
            let r = (p[0].powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt() * crate::AU_KM;
            assert!((356_000.0..407_000.0).contains(&r), "r={r}");
        }
    }

    #[test]
    fn moon_not_sunward_locked() {
        let a = moon_geocentric_ecl(crate::time::JD_J2000);
        let b = moon_geocentric_ecl(crate::time::JD_J2000 + 14.0);
        let dot = a[0] * b[0] + a[1] * b[1];
        assert!(dot < 0.0, "half-orbit apart should point opposite-ish");
    }
}
