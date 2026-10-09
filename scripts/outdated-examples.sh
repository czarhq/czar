#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
export UV_CACHE_DIR="${UV_CACHE_DIR:-$project_root/.czar/uv-cache}"
mkdir -p "$UV_CACHE_DIR"

# shellcheck disable=SC1091
source "$project_root/scripts/deps-common.sh"
require_uv

found=0
for pyproject in examples/workflows/*/pyproject.toml; do
  [ -f "$pyproject" ] || continue
  found=1
  dir="$(dirname "$pyproject")"
  name="$(basename "$dir")"
  echo "==> $name"
  uv tree --project "$dir" --outdated --depth 1 --exclude-newer "$CZAR_UV_EXCLUDE_NEWER"
  echo
done

if [ "$found" -eq 0 ]; then
  echo "No example Python projects found."
fi
