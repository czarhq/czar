#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

if ! command -v cargo-outdated >/dev/null 2>&1; then
  echo "Missing required tool: cargo-outdated" >&2
  echo "Install it with:" >&2
  echo "  cargo install cargo-outdated" >&2
  exit 1
fi

exec cargo outdated --workspace
