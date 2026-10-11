//! Keplerian two-body propagation (PLAN.md §5.1/§5.2): moons, asteroids, comets,
//! exoplanets, and Pluto. Exact two-body solve; uncertainty is carried and displayed.

use serde::{Deserialize, Serialize};

use crate::frames::ecl_to_icrs;

/// Osculating Keplerian elements, heliocentric (or host-centric), ecliptic J2000 frame.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Elements {
    /// semi-major axis [AU]
    pub a: f64,
    /// eccentricity
    pub e: f64,
    /// inclination [deg]
    pub i: f64,
    /// longitude of ascending node [deg]
    pub omega: f64,
    /// argument of perihelion [deg]
    pub w: f64,
    /// mean longitude at epoch [deg]
    pub l0: f64,
    /// epoch of elements [JD TT]
    pub epoch: f64,
    /// mean motion [deg/day]; if 0 it is derived from `a` and `mu_scale`
    pub n: f64,
    /// gravitational parameter scaling (1.0 = Sun). For exoplanet hosts: M_star/M_sun.
    pub mu_scale: f64,
    /// per-century secular rates (a,e,i,Ω,ϖ,λ) for JPL-style tables; zeros otherwise
    pub rates: [f64; 6],
}

impl Elements {
    /// Mean motion from a and mu (deg/day).
    pub fn mean_motion(&self) -> f64 {
        if self.n != 0.0 {
            self.n
        } else {
            0.985_607_668_6 / (self.a * self.a * self.a).sqrt() * self.mu_scale.sqrt()
        }
    }

    /// Elements shifted to time `jd`.
    /// Convention: `rates[5]` (dL/dt per century), when non-zero, is the FULL mean-longitude
    /// rate including mean motion (JPL table style). When the rate set is zero, the mean
    /// longitude is advanced by the derived mean motion instead.
    pub fn at(&self, jd: f64) -> Elements {
        let t = (jd - self.epoch) / 36_525.0;
        let mut e = *self;
        e.a = self.a + self.rates[0] * t;
        e.e = self.e + self.rates[1] * t;
        e.i = self.i + self.rates[2] * t;
        e.omega = self.omega + self.rates[3] * t;
        let dl = if self.rates[5] != 0.0 {
            self.rates[5] * t
        } else {
            self.mean_motion() * (jd - self.epoch)
        };
        e.l0 = self.l0 + dl;
        if self.rates[3] != 0.0 || self.rates[4] != 0.0 {
            // ϖ(t) = ϖ0 + dϖ/dt·t ; ω = ϖ − Ω   (ϖ0 = ω0 + Ω0)
            let peri = self.w + self.omega + self.rates[4] * t;
            e.w = peri - e.omega;
        }
        e.epoch = jd;
        e.rates = [0.0; 6];
        e
    }

    /// Propagate to `jd` (two-body). Returns ICRS-equatorial state [AU], [AU/day].
    pub fn state(&self, jd: f64) -> State {
        let s = self.state_ecl(jd);
        State {
            pos: ecl_to_icrs(s.pos),
            vel: ecl_to_icrs(s.vel),
        }
    }

    /// Same, but in the ecliptic J2000 frame (pre-rotation).
    pub fn state_ecl(&self, jd: f64) -> State {
        let el = self.at(jd);
        let d2r = std::f64::consts::PI / 180.0;
        let n = el.mean_motion() * d2r; // rad/day
        let m = (el.l0 - el.omega - el.w) * d2r + n * (jd - el.epoch);
        let e = el.e;
        // Kepler: E - e sinE = M  (Newton, safeguarded)
        let mut ek = m + e * m.sin();
        for _ in 0..64 {
            let f = ek - e * ek.sin() - m;
            let fp = 1.0 - e * ek.cos();
            let d = f / fp;
            ek -= d;
            if d.abs() < 1e-13 {
                break;
            }
        }
        let (se, ce) = (ek.sin(), ek.cos());
        // perifocal frame: r = [a(cosE−e), b·sinE, 0]; v = (n·a²/r)·[−sinE, √(1−e²)·cosE, 0]
        let b = el.a * (1.0 - e * e).sqrt();
        let r_pf = [el.a * (ce - e), b * se, 0.0];
        let rf = (r_pf[0] * r_pf[0] + r_pf[1] * r_pf[1]).sqrt();
        let kv = n * el.a * el.a / rf;
        let v_pf = [-kv * se, kv * (1.0 - e * e).sqrt() * ce, 0.0];
        // rotation: Rz(-Ω) Rx(-i) Rz(-ω) applied as perifocal→ecliptic
        let (cw, sw) = (el.w.to_radians().cos(), el.w.to_radians().sin());
        let (co, so) = (el.omega.to_radians().cos(), el.omega.to_radians().sin());
        let (ci, si) = (el.i.to_radians().cos(), el.i.to_radians().sin());
        let rot = |v: [f64; 3]| -> [f64; 3] {
            // perifocal → ecliptic J2000
            let x = (co * cw - so * sw * ci) * v[0] + (-co * sw - so * cw * ci) * v[1];
            let y = (so * cw + co * sw * ci) * v[0] + (-so * sw + co * cw * ci) * v[1];
            let z = (sw * si) * v[0] + (cw * si) * v[1];
            [x, y, z]
        };
        State {
            pos: rot(r_pf),
            vel: rot(v_pf),
        }
    }

    /// Orbital period [days]. When a mean motion is carried (SBDB / catalog
    /// bodies) it is authoritative; otherwise derive from a and mu (Kepler III).
    pub fn period_days(&self) -> f64 {
        if self.n != 0.0 {
            360.0 / self.n
        } else {
            365.25 * (self.a * self.a * self.a / self.mu_scale).sqrt()
        }
    }

    /// One closed orbit sampled UNIFORMLY IN ECCENTRIC ANOMALY, returned as
    /// ecliptic J2000 positions [AU] (n+1 points, loop closed).
    ///
    /// Uniform-in-time sampling collapses near perihelion for high-eccentricity
    /// orbits (1P/Halley e≈0.967 drew as two straight chords through the inner
    /// system); uniform-in-E sampling keeps the drawn ellipse smooth for any
    /// e < 1 — the time stamps bunch near perihelion instead, which is exactly
    /// what Kepler's second law wants. The sweep starts half a period before
    /// `jd` so the curve brackets the current epoch like the old baker did.
    pub fn orbit_points_ecl(&self, jd: f64, n: usize) -> Vec<[f64; 3]> {
        let n_deg = self.mean_motion(); // deg/day
        if !(n_deg > 0.0) || !self.a.is_finite() || self.e >= 1.0 {
            return Vec::new();
        }
        let n_rad = n_deg.to_radians();
        let p = std::f64::consts::TAU / n_rad; // days
        let t0 = jd - p / 2.0;
        let el = self.at(t0);
        let e = el.e;
        // Perihelion passage bracketing the window: M(t0) folded to [0, 2π)
        // and stepped back — without this phase the samples are NOT uniform in
        // E and perihelion kinks return (caught by halley_orbit_stays_smooth).
        let d2r = std::f64::consts::PI / 180.0;
        let m0 = ((el.l0 - el.omega - el.w) * d2r).rem_euclid(std::f64::consts::TAU);
        let t_peri = t0 - m0 / n_rad;
        (0..=n)
            .map(|k| {
                let ek = std::f64::consts::TAU * k as f64 / n as f64;
                // M = E − e·sinE (from perihelion); sweeping E 0..2π covers one
                // full period of TIMES [t_peri, t_peri+P], hence one full ellipse.
                let t = t_peri + (ek - e * ek.sin()) / n_rad;
                self.state_ecl(t).pos
            })
            .collect()
    }
}

/// Position + velocity, ICRS-equatorial, AU and AU/day.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
}

impl State {
    pub fn speed_kms(&self) -> f64 {
        (self.vel[0].powi(2) + self.vel[1].powi(2) + self.vel[2].powi(2)).sqrt() * crate::AUPERDAY_KMS
    }
    pub fn r_au(&self) -> f64 {
        (self.pos[0].powi(2) + self.pos[1].powi(2) + self.pos[2].powi(2)).sqrt()
    }
}

/// Osculating elements from a state vector (for the live element readouts, §10.1).
/// `mu_scale` = central mass in solar masses.
pub fn elements_from_state(s_icrs: &State, mu_scale: f64) -> Elements {
    let s = State {
        pos: crate::frames::icrs_to_ecl(s_icrs.pos),
        vel: crate::frames::icrs_to_ecl(s_icrs.vel),
    };
    let r = s.r_au();
    let v2 = s.vel[0].powi(2) + s.vel[1].powi(2) + s.vel[2].powi(2);
    // vis-viva (units: mu_sun = k^2, k = 0.01720209895 AU^1.5/day)
    let k = 0.017_202_098_95 * mu_scale.sqrt();
    let inv_a = 2.0 / r - v2 / (k * k);
    let a = 1.0 / inv_a;
    // angular momentum
    let h = [
        s.pos[1] * s.vel[2] - s.pos[2] * s.vel[1],
        s.pos[2] * s.vel[0] - s.pos[0] * s.vel[2],
        s.pos[0] * s.vel[1] - s.pos[1] * s.vel[0],
    ];
    let hn = (h[0].powi(2) + h[1].powi(2) + h[2].powi(2)).sqrt();
    let e_vec = {
        let rv = s.pos[0] * s.vel[0] + s.pos[1] * s.vel[1] + s.pos[2] * s.vel[2];
        let c = (v2 - k * k / r) / (k * k);
        let d = rv / (k * k);
        [
            c * s.pos[0] - d * s.vel[0],
            c * s.pos[1] - d * s.vel[1],
            c * s.pos[2] - d * s.vel[2],
        ]
    };
    let e = (e_vec[0].powi(2) + e_vec[1].powi(2) + e_vec[2].powi(2)).sqrt();
    let i = (h[2] / hn).acos().to_degrees();
    let nrm = (h[0].powi(2) + h[1].powi(2)).sqrt();
    let omega = if nrm > 1e-12 {
        h[1].atan2(h[0]).to_degrees() + 90.0
    } else {
        0.0
    };
    // node vector (normalised): ẑ×h / |ẑ×h|
    let nv = [-h[1] / nrm, h[0] / nrm, 0.0];
    let w = if e > 1e-9 {
        let cw = (nv[0] * e_vec[0] + nv[1] * e_vec[1] + nv[2] * e_vec[2]) / e;
        let mut w = cw.acos().to_degrees();
        if e_vec[2] < 0.0 {
            w = 360.0 - w;
        }
        w
    } else {
        0.0
    };
    let m = if e > 1e-9 {
        // r = a(1 − e·cos E)  →  cos E = (1 − r/a)/e ; sign(E) = sign(r·v)
        let ce = (1.0 - r / a) / e;
        let mut ee = ce.clamp(-1.0, 1.0).acos();
        if (s.pos[0] * s.vel[0] + s.pos[1] * s.vel[1] + s.pos[2] * s.vel[2]) < 0.0 {
            ee = -ee;
        }
        ee - e * ee.sin()
    } else {
        0.0
    };
    Elements {
        a,
        e,
        i,
        omega: omega.rem_euclid(360.0),
        w,
        l0: (m.to_degrees() + w + omega).rem_euclid(360.0),
        epoch: 0.0,
        n: 0.0,
        mu_scale,
        rates: [0.0; 6],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: elements → state → elements.
    #[test]
    fn kepler_roundtrip() {
        let el = Elements {
            a: 1.523_679,
            e: 0.093_4,
            i: 1.85,
            omega: 49.56,
            w: 286.5,
            l0: 355.45,
            epoch: crate::time::JD_J2000,
            n: 0.0,
            mu_scale: 1.0,
            rates: [0.0; 6],
        };
        let jd = crate::time::JD_J2000 + 1000.0;
        let s = el.state(jd);
        let mut back = elements_from_state(&s, 1.0);
        back.epoch = jd;
        assert!((back.a - el.a).abs() < 1e-9, "a {} vs {}", back.a, el.a);
        assert!((back.e - el.e).abs() < 1e-9);
        assert!((back.i - el.i).abs() < 1e-9);
        // mean longitude should match propagation
        let m_expect = (el.l0 + el.mean_motion() * 1000.0).rem_euclid(360.0);
        let m_back = back.l0;
        let d = (m_back - m_expect).rem_euclid(360.0).min(360.0 - (m_back - m_expect).rem_euclid(360.0));
        assert!(d < 1e-6, "M mismatch {d}");
    }

    #[test]
    fn mars_distance_sane() {
        let el = Elements {
            a: 1.523_679,
            e: 0.093_4,
            i: 1.85,
            omega: 49.56,
            w: 286.5,
            l0: 355.45,
            epoch: crate::time::JD_J2000,
            n: 0.0,
            mu_scale: 1.0,
            rates: [0.0; 6],
        };
        let s = el.state(crate::time::JD_J2000);
        let r = s.r_au();
        assert!((1.38..1.67).contains(&r), "mars r = {r}");
    }

    /// Worst exterior (turning) angle [deg] along a closed polyline.
    fn max_turn_deg(pts: &[[f64; 3]]) -> f64 {
        let ang = |a: [f64; 3], b: [f64; 3]| {
            let d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let na = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
            let nb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
            (d / (na * nb)).clamp(-1.0, 1.0).acos().to_degrees()
        };
        let mut worst = 0.0f64;
        for k in 1..pts.len() - 1 {
            let a = [pts[k][0] - pts[k - 1][0], pts[k][1] - pts[k - 1][1], pts[k][2] - pts[k - 1][2]];
            let b = [pts[k + 1][0] - pts[k][0], pts[k + 1][1] - pts[k][1], pts[k + 1][2] - pts[k][2]];
            worst = worst.max(ang(a, b));
        }
        worst
    }

    /// 1P/Halley (e = 0.968): uniform-in-E sampling must stay smooth even
    /// through perihelion, where the old uniform-in-time loop collapsed into
    /// two long chords (regression, session-4 user report).
    #[test]
    fn halley_orbit_stays_smooth() {
        let halley = Elements {
            a: 17.928_635_049,
            e: 0.967_935_996,
            i: 162.190_530_044,
            omega: 59.098_947_206,
            w: 112.241_431_464,
            l0: (274.382_337_137 + 59.098_947_206 + 112.241_431_464) % 360.0,
            epoch: 2_439_875.5,
            n: 360.0 / 27_728.046_087_904,
            mu_scale: 1.0,
            rates: [0.0; 6],
        };
        let jd = 2_461_322.5;
        let smooth = halley.orbit_points_ecl(jd, 512);
        assert_eq!(smooth.len(), 513);
        let turn = max_turn_deg(&smooth);
        assert!(turn < 4.0, "uniform-E max turn {turn}° — perihelion kink");
        // perihelion distance must actually be resolved on the curve
        let rmin = smooth.iter().map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()).fold(f64::INFINITY, f64::min);
        assert!((rmin - 0.586).abs() < 0.02, "rmin {rmin} AU vs q 0.586");
        // uniform-in-time with the same budget is dramatically worse near the Sun
        let per = halley.period_days();
        let legacy: Vec<[f64; 3]> = (0..=512)
            .map(|k| halley.state_ecl(jd - per / 2.0 + per * k as f64 / 512.0).pos)
            .collect();
        assert!(max_turn_deg(&legacy) > 3.0 * turn, "expected uniform-time collapse");
    }
}
