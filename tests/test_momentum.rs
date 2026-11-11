//! Rust port of every test in `tests/test_momentum.py`.

use momentum::{Direction, Momentum};
use std::sync::Arc;

struct FakeClock {
    now: std::sync::Mutex<f64>,
}

impl FakeClock {
    fn new(start: f64) -> Self {
        FakeClock { now: std::sync::Mutex::new(start) }
    }
    fn advance(&self, seconds: f64) {
        *self.now.lock().unwrap() += seconds;
    }
}

fn fake_clock(start: f64) -> Arc<FakeClock> {
    Arc::new(FakeClock::new(start))
}

fn clock_fn(c: &Arc<FakeClock>) -> momentum::Clock {
    let c = c.clone();
    Arc::new(move || *c.now.lock().unwrap())
}

#[test]
fn push_and_trend_rising() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    for i in 0..10 {
        m.push(100.0 + 10.0 * i as f64, Some(1000.0 + i as f64 * 10.0)).unwrap();
    }
    let t = m.trend();
    assert_eq!(t.direction, Direction::Rising);
    assert!(t.slope > 0.0);
    assert!(t.short_avg > t.long_avg);
}

#[test]
fn trend_falling() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    for i in 0..10 {
        m.push(200.0 - 10.0 * i as f64, Some(1000.0 + i as f64 * 10.0)).unwrap();
    }
    let t = m.trend();
    assert_eq!(t.direction, Direction::Falling);
    assert!(t.slope < 0.0);
}

#[test]
fn trend_steady() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    for i in 0..10 {
        m.push(100.0, Some(1000.0 + i as f64 * 10.0)).unwrap();
    }
    let t = m.trend();
    assert_eq!(t.direction, Direction::Steady);
    assert_eq!(t.slope, 0.0);
}

#[test]
fn short_avg_excludes_old_samples() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    m.push(100.0, Some(1000.0)).unwrap();
    m.push(100.0, Some(1030.0)).unwrap();
    m.push(300.0, Some(1090.0)).unwrap(); // 1030 sample is exactly at the short cutoff
    let t = m.trend();
    assert!((t.short_avg - (100.0 + 300.0) / 2.0).abs() < 1e-9);
    assert!((t.long_avg - (100.0 + 100.0 + 300.0) / 3.0).abs() < 1e-9);
}

#[test]
fn old_samples_pruned() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    m.push(100.0, Some(1000.0)).unwrap();
    m.push(200.0, Some(1400.0)).unwrap(); // first sample is now outside long window
    let t = m.trend();
    assert_eq!(t.long_avg, 200.0);
}

#[test]
fn accelerating_true_when_short_exceeds_long() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    // 5 minutes of low load, then a burst in the last minute
    for i in 0..25 {
        m.push(100.0, Some(1000.0 + i as f64 * 10.0)).unwrap();
    }
    for i in 0..6 {
        m.push(200.0, Some(1250.0 + i as f64 * 10.0)).unwrap();
    }
    assert!(m.accelerating());
}

#[test]
fn accelerating_false_when_steady() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    for i in 0..30 {
        m.push(100.0, Some(1000.0 + i as f64 * 10.0)).unwrap();
    }
    assert!(!m.accelerating());
}

#[test]
fn accelerating_false_with_insufficient_data() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    m.push(100.0, Some(1000.0)).unwrap();
    assert!(!m.accelerating());
}

#[test]
fn default_clock_used_when_ts_none() {
    let c = fake_clock(1000.0);
    let mut m = Momentum::with_clock(60.0, 300.0, Some(clock_fn(&c))).unwrap();
    m.push(50.0, None).unwrap();
    c.advance(10.0);
    m.push(60.0, None).unwrap();
    let t = m.trend();
    assert_eq!(t.direction, Direction::Rising);
    assert!((t.slope - 1.0).abs() < 1e-9);
}

#[test]
fn invalid_windows_rejected() {
    assert!(Momentum::new(0.0, 300.0).is_err());
    assert!(Momentum::new(300.0, 60.0).is_err());
}

#[test]
fn nan_rejected() {
    let mut m = Momentum::new(60.0, 300.0).unwrap();
    assert!(m.push(f64::NAN, Some(1000.0)).is_err());
}

#[test]
fn empty_trend_is_steady() {
    let m = Momentum::new(60.0, 300.0).unwrap();
    let t = m.trend();
    assert_eq!(t.direction, Direction::Steady);
    assert_eq!(t.slope, 0.0);
    assert_eq!(t.short_avg, 0.0);
    assert_eq!(t.long_avg, 0.0);
}
