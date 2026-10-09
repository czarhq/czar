#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

mkdir -p .czar
export DATABASE_URL=${DATABASE_URL:-sqlite://./.czar/czar.db?mode=rwc}
export PYTHONPATH="$ROOT_DIR/python-sdk${PYTHONPATH:+:$PYTHONPATH}"
export CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS=${CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS:-1}

cd crates/czar-web-react
pnpm install --frozen-lockfile
pnpm run dev &
VITE_PID=$!
cd "$ROOT_DIR"

cleanup() {
    kill "$VITE_PID" 2>/dev/null || true
    wait "$VITE_PID" 2>/dev/null || true
}
trap cleanup INT TERM EXIT

cargo run -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- init

echo "Starting API server (SQLite, Ctrl-C to stop)..."
echo "  Frontend: http://127.0.0.1:5175  (Vite dev, HMR enabled)"
cargo run -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- serve
