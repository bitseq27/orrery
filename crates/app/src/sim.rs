//! Simulation clock (PLAN.md §5.3, §8.2): internal time = JD TT.
//! LIVE pins sim_TT to the wall clock; presets/slider give days-per-second rates.

use orrery_astro::time::{jd_to_tt_str, now_jd_tt};

use crate::app_state::AppState;

/// days/second for each preset tab (index = AppState::rate_tab).
pub const TAB_RATES: [f64; 6] = [
    0.0,              // LIVE (wall clock)
    60.0 / 86_400.0,  // 60×
    1.0 / 24.0,       // 1 h/s
    1.0,              // 1 d/s
    365.25,           // 1 yr/s
    36_525.0,         // MAX = 100 yr/s (clamped to validated span)
];

pub struct SimClock {
    pub jd_tt: f64,
    pub live: bool,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            jd_tt: now_jd_tt(),
            live: true,
        }
    }
}

impl SimClock {
    /// Rate in days/second implied by the current UI controls.
    pub fn rate_days_per_sec(st: &AppState) -> f64 {
        if st.rate_tab < 5 {
            TAB_RATES[st.rate_tab]
        } else {
            2f64.powf(st.rate_log as f64)
        }
    }

    /// Advance by wall-clock `dt` seconds. LIVE re-pins to the clock.
    pub fn advance(&mut self, st: &AppState, dt: f32) {
        if st.paused {
            return;
        }
        if st.rate_tab == 0 && !st.rewind {
            self.jd_tt = now_jd_tt();
            self.live = true;
            return;
        }
        self.live = false;
        let mut r = Self::rate_days_per_sec(st);
        if st.rate_tab == 0 {
            r = TAB_RATES[3]; // paused-LIVE with rewind = 1 d/s backwards
        }
        let dir = if st.rewind { -1.0 } else { 1.0 };
        self.jd_tt += r * dt as f64 * dir;
        // clamp to the validated span of the bundled theories (§5.3, §8.2 MAX)
        let (lo, hi) = orrery_astro::planets::validated_span_jd();
        self.jd_tt = self.jd_tt.clamp(lo, hi);
    }

    pub fn epoch_str(&self) -> String {
        jd_to_tt_str(self.jd_tt)
    }
}
