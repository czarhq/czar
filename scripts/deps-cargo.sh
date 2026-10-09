#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/deps-common.sh"

command_name="${1:-}"

case "$command_name" in
  outdated)
    require_cargo_tool outdated cargo-outdated
    cd "$ROOT_DIR"
    temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/czar-cargo-outdated.XXXXXX")"
    trap 'rm -rf "$temp_dir"' EXIT
    ln -s "$ROOT_DIR/crates" "$temp_dir/crates"
    ln -s "$ROOT_DIR/Cargo.lock" "$temp_dir/Cargo.lock"
    if [[ -d "$ROOT_DIR/.cargo" ]]; then
      ln -s "$ROOT_DIR/.cargo" "$temp_dir/.cargo"
    fi
    ln -s "$ROOT_DIR/Cargo.toml" "$temp_dir/Cargo.toml"
    cargo outdated --workspace --manifest-path "$temp_dir/Cargo.toml"
    ;;
  licenses)
    require_cargo_tool deny cargo-deny
    cd "$ROOT_DIR"
    if is_summary; then
      cargo deny check licenses >/dev/null
      echo "rust projects=1 packages=workspace"
    else
      cargo deny check licenses
    fi
    ;;
  vulns)
    require_cargo_tool deny cargo-deny
    cd "$ROOT_DIR"
    cargo deny check advisories -W unmaintained
    ;;
  update)
    # --publish-time is unstable; RUSTC_BOOTSTRAP lets stable cargo accept it.
    for manifest in $(cargo_manifests); do
      echo "==> ${manifest#$ROOT_DIR/} (published before $CZAR_UV_EXCLUDE_NEWER)"
      RUSTC_BOOTSTRAP=1 cargo generate-lockfile \
        -Zunstable-options \
        --publish-time "$CZAR_UV_EXCLUDE_NEWER" \
        --manifest-path "$manifest"
    done
    ;;
  update-crate)
    # One crate to the newest version published before the cutoff (the same
    # 7-day rule as `update`), e.g. for a security fix.
    crate="${2:?usage: $0 update-crate <crate>}"
    cd "$ROOT_DIR"
    for version in $(python3 - "$crate" "$CZAR_UV_EXCLUDE_NEWER" <<'PY'
import json, sys, urllib.request
crate, cutoff = sys.argv[1], sys.argv[2]
req = urllib.request.Request(f"https://crates.io/api/v1/crates/{crate}/versions", headers={"User-Agent": "czar-deps (https://github.com/czarhq/czar)"})
versions = json.load(urllib.request.urlopen(req))["versions"]
for v in versions:  # newest first
    if not v["yanked"] and v["created_at"] < cutoff:
        print(v["num"])
PY
    ); do
      if cargo update -p "$crate" --precise "$version" 2>/dev/null; then
        echo "$crate -> $version (published before $CZAR_UV_EXCLUDE_NEWER)"
        exit 0
      fi
    done
    echo "no version of $crate published before $CZAR_UV_EXCLUDE_NEWER fits the lockfile" >&2
    exit 1
    ;;
  majors)
    cd "$ROOT_DIR"
    python3 ./scripts/deps-cargo-majors.py --cutoff "$CZAR_UV_EXCLUDE_NEWER"
    ;;
  *)
    echo "Usage: $0 {outdated|licenses|vulns|update|update-crate <crate>|majors}" >&2
    exit 1
    ;;
esac
