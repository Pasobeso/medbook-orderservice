# syntax=docker/dockerfile:1
ARG RUST_VERSION=1.90

# ---------- builder base (มี cargo-chef + build tools) ----------
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-slim AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev libpq-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ---------- planner (generate recipe.json) ----------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------- builder (cook deps & build app) ----------
FROM chef AS builder
COPY --from=planner /app/recipe.json /app/recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
ENV RUSTFLAGS="-C strip=debuginfo"
RUN cargo build --frozen --release --bin medbook-orderservice

# ---------- runtime ----------
FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libpq5 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /app/target/release/medbook-orderservice /usr/local/bin/server

RUN useradd --system --home-dir /app --create-home app && chown -R app:app /app
USER app

ENV PORT=3000 RUST_LOG=info RUST_BACKTRACE=1
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/server"]
