# syntax=docker/dockerfile:1.7

# Build stage (dependency-cached via cargo-chef)
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

# Build application
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p czar-cli --release && \
    cp /app/target/release/czar /app/czar

# Runtime stage for local/dev usage. Includes Python + uv so bundled example
# workflows can execute inside the containerized orchestrator.
FROM python:3.13-slim-trixie

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:0.9.24 /uv /uvx /bin/

# Copy the binary and runtime assets from builder
COPY --from=builder /app/czar /app/czar
COPY --from=builder /app/crates/czar-store-postgres/migrations ./migrations
COPY --from=builder /app/examples/workflows ./examples/workflows
COPY --from=builder /app/python-sdk ./python-sdk

ENV DATABASE_URL=postgres://postgres:postgres@postgres:5432/orchestrator
ENV CZAR_WORKFLOW_DEFINITIONS_DIR=/app/examples/workflows
ENV PYTHONPATH=/app/python-sdk

ENTRYPOINT ["/app/czar"]
