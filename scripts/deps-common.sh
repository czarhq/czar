#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT_DIR

required_uv_version="${CZAR_REQUIRED_UV_VERSION:-0.9.24}"
min_release_age_days="${CZAR_MIN_RELEASE_AGE_DAYS:-7}"
export CZAR_MIN_RELEASE_AGE_DAYS="$min_release_age_days"

compute_uv_exclude_newer() {
  python3 - <<'PY'
from datetime import datetime, timedelta, timezone
import os

days = int(os.environ["CZAR_MIN_RELEASE_AGE_DAYS"])
cutoff = (datetime.now(timezone.utc) - timedelta(days=days)).replace(
    hour=0,
    minute=0,
    second=0,
    microsecond=0,
)
print(cutoff.replace(microsecond=0).isoformat().replace("+00:00", "Z"))
PY
}

uv_exclude_newer="${CZAR_UV_EXCLUDE_NEWER:-$(compute_uv_exclude_newer)}"
export CZAR_UV_EXCLUDE_NEWER="$uv_exclude_newer"

python_license_allow_only="${CZAR_PYTHON_LICENSE_ALLOW_ONLY:-Apache Software License;Apache License 2.0;Apache-2.0;BSD License;BSD-2-Clause;BSD-3-Clause;ISC License (ISCL);ISC License;ISC;MIT License;MIT;MPL 2.0;Mozilla Public License 2.0 (MPL 2.0);PSF-2.0;Python Software Foundation License;Python-2.0;The Unlicense (Unlicense);The Unlicense;Zope Public License;zlib/libpng License}"
js_license_allow_only="${CZAR_JS_LICENSE_ALLOW_ONLY:-0BSD;Apache-2.0;Apache-2.0 OR MIT;Apache-2.0 AND MIT;BSD;BSD-2-Clause;BSD-3-Clause;BlueOak-1.0.0;CC-BY-4.0;CDLA-Permissive-2.0;ISC;MIT;MIT OR Apache-2.0;Python-2.0;Unicode-DFS-2016;Unlicense;Zlib}"
export CZAR_PYTHON_LICENSE_ALLOW_ONLY="$python_license_allow_only"
export CZAR_JS_LICENSE_ALLOW_ONLY="$js_license_allow_only"
deps_verbose="${CZAR_DEPS_VERBOSE:-0}"
export CZAR_DEPS_VERBOSE="$deps_verbose"
deps_summary="${CZAR_DEPS_SUMMARY:-0}"
export CZAR_DEPS_SUMMARY="$deps_summary"

is_verbose() {
  [[ "${CZAR_DEPS_VERBOSE:-0}" == "1" ]]
}

is_summary() {
  [[ "${CZAR_DEPS_SUMMARY:-0}" == "1" ]]
}

require_cmd() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "Missing required tool: $name" >&2
    exit 1
  fi
}

require_cargo_tool() {
  local tool="$1"
  local install_name="$2"
  if cargo "$tool" --version >/dev/null 2>&1; then
    return 0
  fi
  echo "Missing required cargo subcommand: cargo $tool" >&2
  echo "Install it with: cargo install --locked $install_name" >&2
  exit 1
}

require_uv() {
  require_cmd uv
  python3 - "$required_uv_version" <<'PY'
import subprocess
import sys

def parse(version: str) -> tuple[int, ...]:
    parts = []
    for item in version.split("."):
        digits = ""
        for char in item:
            if char.isdigit():
                digits += char
            else:
                break
        parts.append(int(digits or "0"))
    return tuple(parts)

required = parse(sys.argv[1])
raw = subprocess.check_output(["uv", "--version"], text=True).strip().split()
current = parse(raw[1])
if current < required:
    print(f"uv {sys.argv[1]} or newer is required; found {raw[1]}", file=sys.stderr)
    sys.exit(1)
PY
}

python_project_dirs() {
  local dir
  for dir in "$ROOT_DIR"/scripts "$ROOT_DIR"/tests/worker-e2e/workflow "$ROOT_DIR"/examples/workflows/*; do
    [[ -f "$dir/uv.lock" ]] || continue
    printf '%s\n' "$dir"
  done
}

cargo_manifests() {
  printf '%s\n' \
    "$ROOT_DIR/Cargo.toml" \
    "$ROOT_DIR/examples/workflows/polyglot/rust_tasks/Cargo.toml"
}

js_project_dirs() {
  printf '%s\n' \
    "$ROOT_DIR/crates/czar-web-react" \
    "$ROOT_DIR/crates/czar-store-d1/wrangler-harness" \
    "$ROOT_DIR/deploy/cloudflare/full"
}
