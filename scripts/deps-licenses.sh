#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_summary() {
  local label="$1"
  shift
  local output
  if ! output="$(CZAR_DEPS_SUMMARY=1 "$@" 2>&1)"; then
    printf '%s\n' "$output"
    return 1
  fi
  printf '%s\n' "$output"
}

if [[ "${CZAR_DEPS_VERBOSE:-0}" == "1" ]]; then
  bash ./scripts/deps-cargo.sh licenses
  bash ./scripts/deps-python.sh licenses
  bash ./scripts/deps-js.sh licenses
  exit 0
fi

rust_summary="$(run_summary rust bash ./scripts/deps-cargo.sh licenses)"
python_summary="$(run_summary python bash ./scripts/deps-python.sh licenses)"
js_summary="$(run_summary js bash ./scripts/deps-js.sh licenses)"

python_projects="$(printf '%s\n' "$python_summary" | awk -F'[ =]' '/^python / {print $3}')"
python_packages="$(printf '%s\n' "$python_summary" | awk -F'[ =]' '/^python / {print $5}')"
js_projects="$(printf '%s\n' "$js_summary" | awk -F'[ =]' '/^js / {print $3}')"
js_packages="$(printf '%s\n' "$js_summary" | awk -F'[ =]' '/^js / {print $5}')"
js_skipped="$(printf '%s\n' "$js_summary" | awk -F'[ =]' '/^js / {print $7}')"

echo "licenses ok: rust workspace; python ${python_projects:-0} project(s), ${python_packages:-0} package(s); js ${js_projects:-0} production project(s), ${js_packages:-0} package(s), ${js_skipped:-0} skipped"
