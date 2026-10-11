//! EXTENDED real small-body catalogue (session 6, rev 3): 1 000+ catalogued
//! asteroids & comets fetched from the JPL SBDB Query API by
//! tools/fetch_population.py — real designations, real osculating elements,
//! real PHA/NEO flags, real H magnitudes / diameters / rotation periods.
//! (Replaces session-5's synthetic population: the HMI carries REAL data only.)
//! The table is family-interleaved (MBA/APO/ATE/AMO/TJN/TNO/IEO/JFC/HTC/comets)
//! and sorted brightest-first inside each family, so the DENSITY slider
//! (20..=1000) always draws a balanced, observationally-prioritised prefix.

use crate::kepler::Elements;
use crate::pop_data::POP;
use crate::small_bodies::{Kind, SmallBody};

fn build() -> Vec<SmallBody> {
    POP.iter()
        .map(
            |(name, kind, epoch, a, e, i, om, w, ma, per, q, pha, neo, h, diam, rot, class)| SmallBody {
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
                pha: *pha == 1,
                neo: *neo == 1,
                h_mag: *h,
                diam_km: *diam,
                rot_h: *rot,
                spec: "",
                class,
            },
        )
        .collect()
}

/// The extended real catalogue (built once, shared). A prefix [0..n] is a
/// balanced family mix — `n` is the DENSITY slider value (20..=1000).
pub fn population() -> &'static Vec<SmallBody> {
    static P: std::sync::OnceLock<Vec<SmallBody>> = std::sync::OnceLock::new();
    P.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::small_bodies::small_bodies;

    #[test]
    fn population_is_real_and_large() {
        let p = population();
        assert!(p.len() >= 1000, "expected ≥1000 real bodies, got {}", p.len());
        for (i, b) in p.iter().enumerate() {
            assert!(p[..i].iter().all(|o| o.name != b.name), "dup {}", b.name);
            assert!(b.el.a > 0.0 && b.el.e < 1.0 && b.el.period_days() > 0.0, "{} elements", b.name);
            assert!(!b.name.starts_with("SYN"), "no synthetic entries allowed");
        }
        // no overlap with the 52-body core table
        for b in p.iter() {
            assert!(
                small_bodies().iter().all(|c| c.name != b.name),
                "{} overlaps core SBDB table",
                b.name
            );
        }
    }

    #[test]
    fn families_and_flags_survive() {
        let p = population();
        let classes: std::collections::HashSet<&str> = p.iter().map(|b| b.class).collect();
        for want in ["MBA", "APO", "TJN", "TNO"] {
            assert!(classes.contains(want), "missing family {want}");
        }
        assert!(p.iter().filter(|b| b.pha).count() >= 20, "real PHAs expected");
        assert!(p.iter().filter(|b| b.kind == Kind::Comet).count() >= 100, "real comets expected");
        // pha implies neo in SBDB semantics
        assert!(p.iter().all(|b| !b.pha || b.neo), "PHA without NEO flag");
    }

    #[test]
    fn prefix_is_a_balanced_mix() {
        let p = population();
        for n in [20usize, 100, 1000] {
            let n = n.min(p.len());
            let comets = p[..n].iter().filter(|b| b.kind == Kind::Comet).count();
            let neos = p[..n].iter().filter(|b| b.neo).count();
            let belt = p[..n].iter().filter(|b| b.class == "MBA").count();
            assert!(belt > 0 && (comets + neos) > 0, "degenerate prefix at {n}");
        }
    }
}
