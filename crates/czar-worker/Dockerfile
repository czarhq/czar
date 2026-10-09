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
    cargo build -p czar-worker --release && \
    cp /app/target/release/czar-worker /app/czar-worker

FROM python:3.13-slim-trixie

WORKDIR /app

COPY --from=ghcr.io/astral-sh/uv:0.9.24 /uv /uvx /bin/

COPY --from=builder /app/czar-worker /app/czar-worker
COPY --from=builder /app/python-sdk ./python-sdk

ENV PYTHONPATH=/app/python-sdk

# workflow.yaml and task assets are volume-mounted at runtime
ENTRYPOINT ["/app/czar-worker"]
CMD ["--addr", "0.0.0.0:8081", "--workflow-path", "/workflow/workflow.yaml", "--working-dir", "/workflow"]
