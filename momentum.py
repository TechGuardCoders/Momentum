"""Momentum: rolling-window load trend detection.

Consumes metric samples (requests/sec, queue depth, ...) over a sliding
window and reports the trend (rising/falling/steady) with slope, plus a
short-vs-long window comparison to detect acceleration — the signal you
want *before* saturation, not after.

Example:
    from momentum import Momentum

    m = Momentum(short_s=60, long_s=300)
    for ts, rps in samples:
        m.push(rps, ts=ts)

    t = m.trend()
    if t["direction"] == "rising" and m.accelerating():
        scale_up()  # load is climbing and climbing faster — act early
"""

from __future__ import annotations

import math
from typing import Callable, Optional


def _default_clock() -> float:
    import time

    return time.time()


class Momentum:
    """Sliding-window load trend tracker.

    Args:
        short_s: Length of the short window in seconds (default 60).
        long_s: Length of the long window in seconds (default 300).
        clock: Injectable clock returning current time in seconds.
            Defaults to ``time.time``. Pass a fake clock in tests.

    Samples older than ``long_s`` are pruned on every push/trend call.
    """

    def __init__(
        self,
        short_s: float = 60.0,
        long_s: float = 300.0,
        clock: Optional[Callable[[], float]] = None,
    ) -> None:
        if short_s <= 0 or long_s <= 0:
            raise ValueError("window lengths must be positive")
        if short_s > long_s:
            raise ValueError("short_s must be <= long_s")
        self.short_s = float(short_s)
        self.long_s = float(long_s)
        self._clock = clock or _default_clock
        # (timestamp, value) pairs, oldest first
        self._samples: list[tuple[float, float]] = []

    def push(self, value: float, ts: Optional[float] = None) -> None:
        """Record a sample. ``ts`` defaults to the injected clock's now."""
        if ts is None:
            ts = self._clock()
        if math.isnan(value):
            raise ValueError("value must not be NaN")
        self._samples.append((float(ts), float(value)))
        self._prune(ts)

    def _prune(self, now: float) -> None:
        cutoff = now - self.long_s
        # Samples are appended in order; drop from the front while stale.
        i = 0
        n = len(self._samples)
        while i < n and self._samples[i][0] < cutoff:
            i += 1
        if i:
            del self._samples[:i]

    def _window(self, seconds: float, now: float) -> list[tuple[float, float]]:
        cutoff = now - seconds
        return [s for s in self._samples if s[0] >= cutoff]

    @staticmethod
    def _avg(samples: list[tuple[float, float]]) -> float:
        if not samples:
            return 0.0
        return sum(v for _, v in samples) / len(samples)

    def trend(self) -> dict:
        """Return trend over the long window.

        Returns a dict with:
            direction: "rising" | "falling" | "steady"
            slope: least-squares slope of value vs time (units/second)
            short_avg: mean value over the short window
            long_avg: mean value over the long window
        """
        now = self._samples[-1][0] if self._samples else self._clock()
        self._prune(now)
        window = self._window(self.long_s, now)
        if len(window) < 2:
            return {
                "direction": "steady",
                "slope": 0.0,
                "short_avg": self._avg(window),
                "long_avg": self._avg(window),
            }

        t0 = window[0][0]
        xs = [ts - t0 for ts, _ in window]
        ys = [v for _, v in window]
        n = float(len(xs))
        mean_x = sum(xs) / n
        mean_y = sum(ys) / n
        var_x = sum((x - mean_x) ** 2 for x in xs)
        if var_x == 0.0:
            slope = 0.0
        else:
            cov = sum((x - mean_x) * (y - mean_y) for x, y in zip(xs, ys))
            slope = cov / var_x

        eps = 1e-12
        if slope > eps:
            direction = "rising"
        elif slope < -eps:
            direction = "falling"
        else:
            direction = "steady"

        short = self._window(self.short_s, now)
        return {
            "direction": direction,
            "slope": slope,
            "short_avg": self._avg(short),
            "long_avg": self._avg(window),
        }

    def accelerating(self) -> bool:
        """True when short-window load is growing faster than the long window.

        Compares the short-window average against the long-window average:
        if the recent average meaningfully exceeds the long-run average,
        load is accelerating. With too little data, returns False.
        """
        now = self._samples[-1][0] if self._samples else self._clock()
        self._prune(now)
        short = self._window(self.short_s, now)
        long_w = self._window(self.long_s, now)
        if len(short) < 2 or len(long_w) < 2:
            return False
        short_avg = self._avg(short)
        long_avg = self._avg(long_w)
        # Require a meaningful relative gap to avoid noise flapping.
        scale = max(abs(long_avg), 1e-9)
        return (short_avg - long_avg) / scale > 0.05
