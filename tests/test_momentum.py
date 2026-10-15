import pytest

from momentum import Momentum


class FakeClock:
    def __init__(self, start: float = 1000.0) -> None:
        self.now = start

    def __call__(self) -> float:
        return self.now

    def advance(self, seconds: float) -> None:
        self.now += seconds


def test_push_and_trend_rising():
    m = Momentum(short_s=60, long_s=300)
    for i in range(10):
        m.push(100.0 + 10 * i, ts=1000.0 + i * 10)
    t = m.trend()
    assert t["direction"] == "rising"
    assert t["slope"] > 0
    assert t["short_avg"] > t["long_avg"]


def test_trend_falling():
    m = Momentum(short_s=60, long_s=300)
    for i in range(10):
        m.push(200.0 - 10 * i, ts=1000.0 + i * 10)
    t = m.trend()
    assert t["direction"] == "falling"
    assert t["slope"] < 0


def test_trend_steady():
    m = Momentum(short_s=60, long_s=300)
    for i in range(10):
        m.push(100.0, ts=1000.0 + i * 10)
    t = m.trend()
    assert t["direction"] == "steady"
    assert t["slope"] == 0.0


def test_short_avg_excludes_old_samples():
    m = Momentum(short_s=60, long_s=300)
    m.push(100.0, ts=1000.0)
    m.push(100.0, ts=1030.0)
    m.push(300.0, ts=1090.0)  # 1030 sample is exactly at the short cutoff
    t = m.trend()
    assert t["short_avg"] == pytest.approx((100.0 + 300.0) / 2)
    assert t["long_avg"] == pytest.approx((100.0 + 100.0 + 300.0) / 3)


def test_old_samples_pruned():
    m = Momentum(short_s=60, long_s=300)
    m.push(100.0, ts=1000.0)
    m.push(200.0, ts=1400.0)  # first sample is now outside long window
    t = m.trend()
    assert t["long_avg"] == 200.0


def test_accelerating_true_when_short_exceeds_long():
    m = Momentum(short_s=60, long_s=300)
    # 5 minutes of low load, then a burst in the last minute
    for i in range(25):
        m.push(100.0, ts=1000.0 + i * 10)
    for i in range(6):
        m.push(200.0, ts=1250.0 + i * 10)
    assert m.accelerating() is True


def test_accelerating_false_when_steady():
    m = Momentum(short_s=60, long_s=300)
    for i in range(30):
        m.push(100.0, ts=1000.0 + i * 10)
    assert m.accelerating() is False


def test_accelerating_false_with_insufficient_data():
    m = Momentum(short_s=60, long_s=300)
    m.push(100.0, ts=1000.0)
    assert m.accelerating() is False


def test_default_clock_used_when_ts_none():
    clock = FakeClock()
    m = Momentum(short_s=60, long_s=300, clock=clock)
    m.push(50.0)
    clock.advance(10)
    m.push(60.0)
    t = m.trend()
    assert t["direction"] == "rising"
    assert t["slope"] == pytest.approx(1.0)


def test_invalid_windows_rejected():
    with pytest.raises(ValueError):
        Momentum(short_s=0, long_s=300)
    with pytest.raises(ValueError):
        Momentum(short_s=300, long_s=60)


def test_nan_rejected():
    m = Momentum()
    with pytest.raises(ValueError):
        m.push(float("nan"), ts=1000.0)


def test_empty_trend_is_steady():
    m = Momentum()
    t = m.trend()
    assert t["direction"] == "steady"
    assert t["slope"] == 0.0
    assert t["short_avg"] == 0.0
    assert t["long_avg"] == 0.0
