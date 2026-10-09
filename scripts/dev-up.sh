#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

export HOST_UID="${HOST_UID:-$(id -u)}"
export HOST_GID="${HOST_GID:-$(id -g)}"

echo "Starting Dockerized Czar stack..."
echo "  Frontend: http://127.0.0.1:5175"
echo "  API:      http://127.0.0.1:4000"
exec docker compose -f crates/czar-cli/docker-compose.dev.yml up --build
