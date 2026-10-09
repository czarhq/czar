#!/usr/bin/env bash
set -euo pipefail

project_root="$(git rev-parse --show-toplevel)"
cd "$project_root"
mkdir -p .czar
export DATABASE_URL=${DATABASE_URL:-sqlite://$project_root/.czar/czar.db?mode=rwc}
export CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS=${CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS:-1}
cargo run -q -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- init >/dev/null 2>&1 || true
echo "Starting scheduler + web API (SQLite) on http://127.0.0.1:4000 ..."
cargo run -q -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- serve &
SERVER_PID=$!
cleanup() { kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; }
trap cleanup INT TERM EXIT
sleep 2
cargo run -p czar-tui -- --base-url http://127.0.0.1:4000
