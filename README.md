# Momentum

Rolling-window load trend tracker. Feed it metric samples (requests/sec,
queue depth, connection count...) and it tells you whether load is
**rising, falling, or steady** — with a fitted slope — and whether load is
**accelerating** by comparing a short window against a long one.

The point: catch the *shape* of load growth early enough to scale
proactively, before saturation.

## Results

- Pure Python, zero runtime dependencies. Injectable clock — fully
  deterministic in tests, no sleeps, no wall-clock flakiness.
- 13 tests, all passing in CI on every push.
- **Rust port** (`src/lib.rs`): same algorithm, same semantics, zero
  dependencies — for embedding in perf-critical collectors. 13 tests in
  `tests/test_momentum.rs` mirror the Python suite 1:1; `cargo test` runs
  in CI alongside pytest.

## Architecture

```
Momentum(short_s=60, long_s=300, clock=None)
  push(value, ts=None)   # record a sample; ts defaults to the clock
  trend()                # {direction, slope, short_avg, long_avg}
  accelerating()         # short avg growing faster than long avg
```

- **Sliding windows**: samples are kept for `long_s` seconds and pruned on
  every push; `short_s` is the recent slice used for acceleration checks.
- **Trend**: ordinary least-squares slope of value vs. time over the long
  window, classified into rising/falling/steady.
- **Acceleration**: relative gap between short-window and long-window
  averages, with a noise floor so a single spike doesn't flap the signal.

## Why

Threshold alerts fire *after* load crosses a line. Momentum watches the
*derivative*: a flat 80% CPU is fine; 40%→70% in two minutes is not.
Short-vs-long comparison is the same instinct an operator has when
glancing at a dashboard — encoded so automation can act on it.

## Usage

Python:

```python
from momentum import Momentum

m = Momentum(short_s=60, long_s=300)
for rps, ts in metric_stream:
    m.push(rps, ts=ts)

t = m.trend()
if t["direction"] == "rising" and m.accelerating():
    scale_out()  # climbing, and climbing faster — act now
```

Rust:

```rust
use momentum::{Momentum, Direction};

let mut m = Momentum::new(60.0, 300.0)?;
m.push(rps, Some(ts))?;
let t = m.trend();
if t.direction == Direction::Rising && m.accelerating() {
    scale_out(); // climbing, and climbing faster — act now
}
```

## Development

```bash
uv venv && . .venv/Scripts/activate  # or source .venv/bin/activate
uv pip install pytest
pytest

cargo test   # Rust port
```

## Docker

Multi-stage, multi-arch image (linux/amd64 + linux/arm64) built on `python:3.12-slim`:

```bash
docker build -t momentum .
docker run --rm momentum   # runs the test suite as a smoke check
```

For the Go/Rust variant, see the note at the top of the Dockerfile.
