#!/usr/bin/env bash
set -euo pipefail

name="${1:?example name is required}"
profile="${2:-debug}"
project_root="$(pwd)"

python3 ./scripts/examples.py assert-runnable "$name"

# The manifest knows where each example lives (some are under docs/).
example_dir="$(python3 ./scripts/examples.py resolve-dir "$name")"

if [[ -f "$example_dir/build.py" ]]; then
  python3 "$example_dir/build.py" --profile "$profile"
  exit 0
fi

if [[ -f "$example_dir/build.sh" ]]; then
  bash "$example_dir/build.sh" "$profile"
  exit 0
fi

exit 0
