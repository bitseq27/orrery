//! Time base (PLAN.md §5.3): internal sim-time = JD (Terrestrial Time).

/// Julian date of J2000.0 epoch (2000-01-01 12:00 TT).
pub const JD_J2000: f64 = 2_451_545.0;
/// TAI−UTC leap-second table (JD_UTC of introduction, TAI−UTC seconds), 1972→2017.
/// No leap second has been introduced since 2017-01-01; the last entry is extrapolated
/// forward (documented in UI footer, §5.3).
const LEAP_TABLE: [(f64, f64); 27] = [
    (2441317.5, 10.0), (2441683.5, 11.0), (2442048.5, 12.0), (2442413.5, 13.0),
    (2442778.5, 14.0), (2443144.5, 15.0), (2443509.5, 16.0), (2443874.5, 17.0),
    (2444239.5, 18.0), (2444604.5, 19.0), (2444786.5, 20.0), (2445151.5, 21.0),
    (2445516.5, 22.0), (2446246.5, 23.0), (2447161.5, 24.0), (2447892.5, 25.0),
    (2448257.5, 26.0), (2448622.5, 27.0), (2448987.5, 28.0), (2449352.5, 29.0),
    (2450083.5, 30.0), (2450630.5, 31.0), (2451179.5, 32.0), (2453736.5, 33.0),
    (2454832.5, 34.0), (2456109.5, 35.0), (2457204.5, 36.0),
];
/// TAI−UTC today (since 2017-01-01).
pub const TAI_MINUS_UTC_S: f64 = 37.0;
/// TT−TAI (constant).
pub const TT_MINUS_TAI_S: f64 = 32.184;
/// TT − UTC in seconds at the present epoch (37 + 32.184 = 69.184).
pub const TT_MINUS_UTC_S: f64 = TAI_MINUS_UTC_S + TT_MINUS_TAI_S;

/// TT−UTC [s] for a given JD (TT), from the leap-second table.
pub fn tt_minus_utc(jd_tt: f64) -> f64 {
    let jd_utc_approx = jd_tt - TT_MINUS_UTC_S / 86_400.0;
    let mut tai_utc = 10.0;
    for &(jd_ls, off) in LEAP_TABLE.iter() {
        if jd_utc_approx >= jd_ls {
            tai_utc = off;
        }
    }
    if jd_utc_approx >= 2457754.5 {
        tai_utc = TAI_MINUS_UTC_S; // 2017-01-01 →
    }
    if jd_utc_approx < LEAP_TABLE[0].0 {
        return TT_MINUS_TAI_S + 10.0; // pre-1972: extrapolated, flagged in docs
    }
    tai_utc + TT_MINUS_TAI_S
}
/// Days per Julian century.
pub const DAYS_PER_CENTURY: f64 = 36_525.0;

/// Current wall-clock as JD in Terrestrial Time.
pub fn now_jd_tt() -> f64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    unix_to_jd_tt(secs)
}

pub fn unix_to_jd_tt(unix_secs: f64) -> f64 {
    unix_secs / 86_400.0 + 2_440_587.5 + TT_MINUS_UTC_S / 86_400.0
}

pub fn jd_tt_to_unix(jd_tt: f64) -> f64 {
    (jd_tt - 2_440_587.5 - tt_minus_utc(jd_tt) / 86_400.0) * 86_400.0
}

/// Centuries since J2000 (TT).
pub fn t_centuries(jd_tt: f64) -> f64 {
    (jd_tt - JD_J2000) / DAYS_PER_CENTURY
}

/// JD TT → proleptic Gregorian calendar date (UTC), returned as (y, m, d, h, mi, s).
pub fn jd_to_calendar(jd_tt: f64) -> (i64, u32, u32, u32, u32, u32) {
    let jd_utc = jd_tt - tt_minus_utc(jd_tt) / 86_400.0;
    let z = (jd_utc + 0.5).floor();
    let f = jd_utc + 0.5 - z;
    let z = z as i64;
    let a = if z < 2_299_161 {
        z
    } else {
        let alpha = ((z as f64 - 1_867_216.25) / 36_524.25).floor() as i64;
        z + 1 + alpha - alpha / 4
    };
    let b = a + 1524;
    let c = ((b as f64 - 122.1) / 365.25).floor() as i64;
    let d = (365.25 * c as f64).floor() as i64;
    let e = ((b as f64 - d as f64) / 30.6001).floor() as i64;
    let day_f = b as f64 - d as f64 - (30.6001 * e as f64).floor() + f;
    let day = day_f.floor() as u32;
    let frac = day_f - day as f64;
    let month = if e < 14 { e - 1 } else { e - 13 } as u32;
    let year = if month > 2 { c - 4716 } else { c - 4715 };
    let secs = (frac * 86_400.0).round() as u32;
    (year, month, day, secs / 3600, (secs / 60) % 60, secs % 60)
}

/// `2026-10-09 12:34:56` (UTC).
pub fn jd_to_utc_str(jd_tt: f64) -> String {
    let (y, mo, d, h, mi, s) = jd_to_calendar(jd_tt);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// TT epoch string for the header (§9): `YYYY-MM-DD hh:mm:ss TT`.
pub fn jd_to_tt_str(jd_tt: f64) -> String {
    let z = (jd_tt + 0.5).floor();
    let f = jd_tt + 0.5 - z;
    let secs = (f * 86_400.0).round() as u32;
    let (y, mo, d, _, _, _) = jd_to_calendar(jd_tt); // date part (UTC-day numbering of the TT instant)
    format!(
        "{y:04}-{mo:02}-{d:02} {:02}:{:02}:{:02} TT",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2000_roundtrip() {
        let (y, m, d, h, mi, s) = jd_to_calendar(JD_J2000);
        assert_eq!((y, m, d), (2000, 1, 1));
        assert_eq!((h, mi, s), (11, 58, 56)); // 12:00:00 TT == 11:58:55.8 UTC → rounds up
    }

    #[test]
    fn known_date() {
        // 2026-10-09 00:00 UTC = JD 2461322.5 (UTC)
        let jd = 2_461_322.5 + TT_MINUS_UTC_S / 86_400.0;
        assert_eq!(&jd_to_utc_str(jd), "2026-10-09 00:00:00");
    }
}
