#!/usr/bin/env bash
set -euo pipefail

name="${1:?example name is required}"
project_root="$(pwd)"

python3 ./scripts/examples.py assert-runnable "$name"

if [[ -z "${DATABASE_URL:-}" ]]; then
  example_db_dir="$project_root/.czar/examples/$name"
  mkdir -p "$example_db_dir"
  rm -f "$example_db_dir/czar.db" "$example_db_dir/czar.db-shm" "$example_db_dir/czar.db-wal"
  export DATABASE_URL="sqlite://$example_db_dir/czar.db?mode=rwc"
else
  mkdir -p .czar
fi

cargo run -q -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- init >/dev/null 2>&1 || true

example_dir="$(python3 ./scripts/examples.py resolve-dir "$name")"
file="$(python3 ./scripts/examples.py resolve-workflow "$name")"

source ./scripts/prepare-example-env.sh "$example_dir" "$project_root"

if [[ -f "$example_dir/build.py" ]]; then
  example_build_profile="${CZAR_VAR_BUILD_PROFILE:-debug}"
  python3 "$example_dir/build.py" --profile "$example_build_profile" --check
fi

cargo run -p czar-cli --no-default-features --features sqlite,local-executors --bin czar -- run "$file"
