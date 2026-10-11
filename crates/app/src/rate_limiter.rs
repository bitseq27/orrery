//! Global request rate limiter (PLAN §5.4, R14/R15).
//!
//! ADR-007: §5.4 prescribes a token bucket (`rate = CPM/60`, `burst = min(CPM,10)`),
//! but §14's acceptance test demands **≤ CPM requests in ANY 60 s window**. A bucket
//! with burst > 1 plus steady refill violates that (5 burst + 5 refilled inside the
//! same window = 9 > 5 at CPM = 5). The stricter contract wins: this is a sliding
//! 60 s window counter capped at CPM. `burst` survives only as the UI gauge scale
//! (min(CPM,10) segments showing remaining window credit).
//! Time is injected so the harness runs a simulated window.

use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct TokenBucket {
    pub cpm: f64,
    stamps: VecDeque<f64>,
    pub last_s: f64,
    pub issued_total: u64,
}

impl TokenBucket {
    pub fn new(cpm: u32, now_s: f64) -> Self {
        Self { cpm: cpm.max(1) as f64, stamps: VecDeque::new(), last_s: now_s, issued_total: 0 }
    }

    pub fn set_cpm(&mut self, cpm: u32, now_s: f64) {
        self.purge(now_s);
        self.cpm = cpm.max(1) as f64;
    }

    fn purge(&mut self, now_s: f64) {
        self.last_s = now_s;
        while self.stamps.front().map(|s| now_s - s >= 60.0).unwrap_or(false) {
            self.stamps.pop_front();
        }
    }

    /// Remaining credit in the current 60 s window.
    pub fn remaining(&self) -> f64 {
        (self.cpm - self.stamps.len() as f64).max(0.0)
    }

    /// UI gauge scale (§5.4 burst semantics kept for display).
    pub fn burst(&self) -> f64 {
        self.cpm.min(10.0)
    }

    /// Gauge fraction 0..1 for the segmented bar.
    pub fn gauge_frac(&self) -> f64 {
        (self.remaining() / self.burst()).clamp(0.0, 1.0)
    }

    /// Seconds until the next request would be admitted (0.0 = now).
    pub fn wait_s(&mut self, now_s: f64) -> f64 {
        self.purge(now_s);
        if (self.stamps.len() as f64) < self.cpm {
            return 0.0;
        }
        let oldest = *self.stamps.front().unwrap_or(&now_s);
        (60.0 - (now_s - oldest)).max(0.0)
    }

    pub fn try_acquire(&mut self, now_s: f64) -> bool {
        self.purge(now_s);
        if (self.stamps.len() as f64) < self.cpm {
            self.stamps.push_back(now_s);
            self.issued_total += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §14 M4.5 acceptance: with CPM = 5, a burst of 30 forced refreshes issues
    /// ≤ 5 in any 60 s window.
    #[test]
    fn cpm5_burst30_stays_under_cap() {
        let mut b = TokenBucket::new(5, 0.0);
        let mut t = 0.0;
        let mut stamps: Vec<f64> = Vec::new();
        for _ in 0..30 {
            loop {
                if b.try_acquire(t) {
                    stamps.push(t);
                    break;
                }
                t += b.wait_s(t).max(0.01);
            }
        }
        for win in 0..400 {
            let w = win as f64 * 1.0;
            let n = stamps.iter().filter(|s| **s >= w && **s < w + 60.0).count();
            assert!(n <= 5, "window @{w}: {n} > 5");
        }
        assert_eq!(stamps.len(), 30);
        // steady-state spacing = 60/CPM
        assert!((stamps[5] - 60.0).abs() < 1e-9);
    }

    #[test]
    fn window_slides() {
        let mut b = TokenBucket::new(20, 0.0);
        for _ in 0..20 {
            assert!(b.try_acquire(0.0));
        }
        assert!(!b.try_acquire(0.0));
        assert!(!b.try_acquire(59.9));
        assert!(b.try_acquire(60.0), "window slides: first stamp expires");
        assert_eq!(b.issued_total, 21);
    }
}
