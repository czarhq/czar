# syntax=docker/dockerfile:1.7

FROM lukemathwalker/cargo-chef:latest-rust-1.95.0 AS chef

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p czar-cli --release --no-default-features --features d1,process \
    && cp /app/target/release/czar /app/czar

FROM python:3.13-slim-trixie

COPY --from=ghcr.io/astral-sh/uv:0.9.24 /uv /uvx /bin/

WORKDIR /app

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    XDG_CACHE_HOME=/home/nonroot/.cache

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 65532 --create-home --home-dir /home/nonroot --shell /usr/sbin/nologin nonroot

COPY --from=builder --chown=65532:65532 /app/czar /app/czar
COPY --from=builder --chown=65532:65532 /app/examples/workflows/simple /app/cloudflare-workflows/simple
COPY --from=builder --chown=65532:65532 /app/examples/workflows/fast /app/cloudflare-workflows/fast

USER 65532:65532
EXPOSE 8080

ENTRYPOINT ["/app/czar", "serve", "--addr", "0.0.0.0:8080"]
