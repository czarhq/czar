#!/usr/bin/env bash
# Build the web UI and copy it into crates/czar-web/ui, so the czar-web crate
# (and everything installed from crates.io) carries it. Run before publishing;
# scripts/publish-prep.sh does. `scripts/embed-ui.sh --clean` removes it again.
set -euo pipefail
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
UI_DIR="$ROOT_DIR/crates/czar-web/ui"
rm -rf "$UI_DIR"
if [[ "${1:-}" == "--clean" ]]; then
  exit 0
fi
pnpm --dir "$ROOT_DIR/crates/czar-web-react" install --frozen-lockfile
pnpm --dir "$ROOT_DIR/crates/czar-web-react" run build
cp -r "$ROOT_DIR/crates/czar-web-react/dist" "$UI_DIR"
echo "Embedded the web UI: $(find "$UI_DIR" -type f | wc -l) files, $(du -sh "$UI_DIR" | cut -f1)"
