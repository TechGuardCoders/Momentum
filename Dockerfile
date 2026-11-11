# Multi-stage, multi-arch (linux/amd64, linux/arm64) — base images are official
# multi-arch builds, so `docker buildx build --platform linux/amd64,linux/arm64` works as-is.
#
# NOTE (Rust variant): Momentum also ships a Rust port (src/lib.rs). To build/test
# that instead, swap the builder stage for:
#   FROM rust:1.82-slim AS rust-builder
#   WORKDIR /app
#   COPY Cargo.toml Cargo.lock ./
#   COPY src ./src
#   COPY tests ./tests
#   RUN cargo test --locked
# The runtime stage below stays unchanged.

FROM python:3.12-slim AS builder
WORKDIR /app
COPY momentum.py pytest.ini ./
COPY tests ./tests
RUN pip install --no-cache-dir pytest

FROM python:3.12-slim
WORKDIR /app
COPY --from=builder /usr/local/lib/python3.12/site-packages /usr/local/lib/python3.12/site-packages
COPY --from=builder /app /app
# Library repo: default command runs the test suite as a smoke check.
CMD ["python", "-m", "pytest", "-q"]
