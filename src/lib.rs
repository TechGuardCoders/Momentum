//! Momentum: rolling-window load trend detection (Rust port of the Python
//! reference implementation in `momentum.py`).
//!
//! Consumes metric samples (requests/sec, queue depth, ...) over a sliding
//! window and reports the trend (rising/falling/steady) with slope, plus a
//! short-vs-long window comparison to detect acceleration — the signal you
//! want *before* saturation, not after.
//!
//! ```
//! use momentum::Momentum;
//!
//! let mut m = Momentum::new(60.0, 300.0).unwrap();
//! m.push(100.0, Some(1000.0));
//! m.push(120.0, Some(1010.0));
//! let t = m.trend();
//! assert_eq!(t.direction, momentum::Direction::Rising);
//! ```

use std::sync::Arc;

/// Trend direction over the long window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Rising,
    Falling,
    Steady,
}

/// Trend over the long window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trend {
    pub direction: Direction,
    /// Least-squares slope of value vs. time (units/second).
    pub slope: f64,
    /// Mean value over the short window.
    pub short_avg: f64,
    /// Mean value over the long window.
    pub long_avg: f64,
}

/// Injectable clock returning current time in seconds.
pub type Clock = Arc<dyn Fn() -> f64 + Send + Sync>;

/// Sliding-window load trend tracker.
pub struct Momentum {
    short_s: f64,
    long_s: f64,
    clock: Option<Clock>,
    // (timestamp, value) pairs, oldest first
    samples: Vec<(f64, f64)>,
}

fn default_clock() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

impl Momentum {
    /// Create a tracker. `short_s` and `long_s` are window lengths in seconds;
    /// `short_s` must be positive and `<= long_s`.
    pub fn new(short_s: f64, long_s: f64) -> Result<Self, String> {
        Self::with_clock(short_s, long_s, None)
    }

    /// Create a tracker with an injectable clock (for deterministic tests).
    pub fn with_clock(short_s: f64, long_s: f64, clock: Option<Clock>) -> Result<Self, String> {
        if short_s <= 0.0 || long_s <= 0.0 {
            return Err("window lengths must be positive".to_string());
        }
        if short_s > long_s {
            return Err("short_s must be <= long_s".to_string());
        }
        Ok(Momentum {
            short_s,
            long_s,
            clock,
            samples: Vec::new(),
        })
    }

    /// Record a sample. `ts` defaults to the injected clock's now.
    pub fn push(&mut self, value: f64, ts: Option<f64>) -> Result<(), String> {
        if value.is_nan() {
            return Err("value must not be NaN".to_string());
        }
        let ts = ts.unwrap_or_else(|| self.now());
        self.samples.push((ts, value));
        self.prune(ts);
        Ok(())
    }

    fn now(&self) -> f64 {
        self.clock.as_ref().map(|c| c()).unwrap_or_else(default_clock)
    }

    fn prune(&mut self, now: f64) {
        let cutoff = now - self.long_s;
        let i = self.samples.iter().take_while(|s| s.0 < cutoff).count();
        self.samples.drain(..i);
    }

    fn window(&self, seconds: f64, now: f64) -> Vec<(f64, f64)> {
        let cutoff = now - seconds;
        self.samples.iter().copied().filter(|s| s.0 >= cutoff).collect()
    }

    fn avg(samples: &[(f64, f64)]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        samples.iter().map(|s| s.1).sum::<f64>() / samples.len() as f64
    }

    /// Return trend over the long window: direction, slope, short_avg, long_avg.
    pub fn trend(&self) -> Trend {
        let now = self.samples.last().map(|s| s.0).unwrap_or_else(|| self.now());
        let window = self.window(self.long_s, now);
        if window.len() < 2 {
            let a = Self::avg(&window);
            return Trend {
                direction: Direction::Steady,
                slope: 0.0,
                short_avg: a,
                long_avg: a,
            };
        }

        let t0 = window[0].0;
        let xs: Vec<f64> = window.iter().map(|s| s.0 - t0).collect();
        let ys: Vec<f64> = window.iter().map(|s| s.1).collect();
        let n = xs.len() as f64;
        let mean_x = xs.iter().sum::<f64>() / n;
        let mean_y = ys.iter().sum::<f64>() / n;
        let var_x: f64 = xs.iter().map(|x| (x - mean_x).powi(2)).sum();
        let slope = if var_x == 0.0 {
            0.0
        } else {
            let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mean_x) * (y - mean_y)).sum();
            cov / var_x
        };

        let eps = 1e-12;
        let direction = if slope > eps {
            Direction::Rising
        } else if slope < -eps {
            Direction::Falling
        } else {
            Direction::Steady
        };

        let short = self.window(self.short_s, now);
        Trend {
            direction,
            slope,
            short_avg: Self::avg(&short),
            long_avg: Self::avg(&window),
        }
    }

    /// True when short-window load is growing faster than the long window.
    ///
    /// Compares the short-window average against the long-window average:
    /// if the recent average meaningfully exceeds the long-run average,
    /// load is accelerating. With too little data, returns false.
    pub fn accelerating(&self) -> bool {
        let now = self.samples.last().map(|s| s.0).unwrap_or_else(|| self.now());
        let short = self.window(self.short_s, now);
        let long_w = self.window(self.long_s, now);
        if short.len() < 2 || long_w.len() < 2 {
            return false;
        }
        let short_avg = Self::avg(&short);
        let long_avg = Self::avg(&long_w);
        // Require a meaningful relative gap to avoid noise flapping.
        let scale = long_avg.abs().max(1e-9);
        (short_avg - long_avg) / scale > 0.05
    }
}
