#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

frontend_dir="crates/czar-web-react"

if ! command -v pnpm >/dev/null 2>&1; then
  echo "Missing required tool: pnpm" >&2
  exit 1
fi

echo "==> czar-web-react"
set +e
pnpm outdated --dir "$frontend_dir"
status=$?
set -e

case "$status" in
  0|1)
    exit 0
    ;;
  *)
    exit "$status"
    ;;
esac
