#!/usr/bin/env bash
# Build the czar-cli wheel for this machine: the `czar` binary (with the web
# UI embedded) in a platform wheel, under python-cli/dist. The release
# workflow builds the same wheel for every platform.
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
"$ROOT_DIR/scripts/embed-ui.sh"
rm -rf "$ROOT_DIR/python-cli/dist"
cd "$ROOT_DIR/python-cli"
uvx --from 'maturin>=1.9,<2' maturin build --release --locked --out dist
