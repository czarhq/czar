#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/deps-common.sh"

command_name="${1:-}"
require_uv

run_outdated() {
  local dir status=0
  for dir in $(python_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    if ! uv tree \
      --project "$dir" \
      --frozen \
      --outdated \
      --depth 1
    then
      status=1
    fi
    echo
  done
  return "$status"
}

run_licenses() {
  local dir python_bin status=0 sync_log report_file project_count=0 package_count=0
  for dir in $(python_project_dirs); do
    if ! is_summary; then
      echo "==> ${dir#$ROOT_DIR/}"
    fi
    sync_log="$(mktemp)"
    if is_verbose; then
      if ! uv sync \
        --project "$dir" \
        --frozen
      then
        status=1
        rm -f "$sync_log"
        if ! is_summary; then
          echo
        fi
        continue
      fi
    else
      if ! uv sync \
        --project "$dir" \
        --frozen \
        >"$sync_log" 2>&1
      then
        status=1
        cat "$sync_log"
        rm -f "$sync_log"
        if ! is_summary; then
          echo
        fi
        continue
      fi
    fi
    rm -f "$sync_log"
    python_bin="$dir/.venv/bin/python"
    report_file="$(mktemp)"
    if ! uvx --from pip-licenses==5.5.0 pip-licenses \
      --python "$python_bin" \
      --allow-only "$CZAR_PYTHON_LICENSE_ALLOW_ONLY" \
      --partial-match \
      --format=json \
      >"$report_file"
    then
      status=1
      cat "$report_file"
      rm -f "$report_file"
      if ! is_summary; then
        echo
      fi
      continue
    fi
    project_count=$((project_count + 1))
    package_count=$(
      python3 - "$report_file" <<'PY'
import json, sys
packages = json.load(open(sys.argv[1], encoding="utf-8"))
print(len(packages))
PY
    )
    if is_summary; then
      package_count="${package_count}"
      total_packages="${total_packages:-0}"
      total_packages=$((total_packages + package_count))
    elif is_verbose; then
      python3 - "$report_file" <<'PY'
import json, sys
packages = json.load(open(sys.argv[1], encoding="utf-8"))
if not packages:
    print("ok: 0 third-party packages")
    raise SystemExit(0)
name_key = "Name" if "Name" in packages[0] else "name"
version_key = "Version" if "Version" in packages[0] else "version"
license_key = "License" if "License" in packages[0] else "license"
for package in packages:
    print(f"{package.get(name_key, '?')} {package.get(version_key, '?')} :: {package.get(license_key, '?')}")
PY
    else
      python3 - "$report_file" <<'PY'
import json, sys
packages = json.load(open(sys.argv[1], encoding="utf-8"))
licenses = sorted({(pkg.get("License") or pkg.get("license") or "?") for pkg in packages})
print(f"ok: {len(packages)} third-party package(s); licenses: {', '.join(licenses) if licenses else 'none'}")
PY
    fi
    rm -f "$report_file"
    if ! is_summary; then
      echo
    fi
  done
  if is_summary && [[ "$status" -eq 0 ]]; then
    echo "python projects=${project_count} packages=${total_packages:-0}"
  fi
  return "$status"
}

# The age gate lives in each pyproject.toml ([tool.uv] exclude-newer), so uv
# enforces it on every resolve. Check that it is declared and that the lockfile
# was resolved under it; passing --exclude-newer here would override the
# relative span and force a re-lock.
run_freshness() {
  local dir status=0
  for dir in $(python_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    if ! grep -qx "exclude-newer = \"${CZAR_MIN_RELEASE_AGE_DAYS} days\"" "$dir/pyproject.toml"; then
      echo "missing [tool.uv] exclude-newer = \"${CZAR_MIN_RELEASE_AGE_DAYS} days\" in pyproject.toml" >&2
      status=1
      continue
    fi
    if ! uv lock --project "$dir" --check; then
      status=1
    fi
  done
  return "$status"
}

# Re-resolve every lockfile to the newest versions the manifest ranges and
# the pyproject age gate allow.
run_update() {
  local dir status=0
  for dir in $(python_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    if ! uv lock --project "$dir" --upgrade; then
      status=1
    fi
  done
  return "$status"
}

run_vulns() {
  local dir requirements_file status=0
  for dir in $(python_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    requirements_file="$(mktemp)"
    if ! uv export --color never \
      --project "$dir" \
      --frozen \
      --format requirements.txt \
      --no-header \
      --no-hashes \
      --no-emit-project \
      --no-emit-local \
      > "$requirements_file"
    then
      rm -f "$requirements_file"
      status=1
      echo
      continue
    fi
    if ! uvx --from pip-audit==2.9.0 pip-audit -r "$requirements_file" --strict; then
      status=1
    fi
    rm -f "$requirements_file"
    echo
  done
  return "$status"
}

case "$command_name" in
  outdated) run_outdated ;;
  licenses) run_licenses ;;
  freshness) run_freshness ;;
  vulns) run_vulns ;;
  update) run_update ;;
  *)
    echo "Usage: $0 {outdated|licenses|freshness|vulns|update}" >&2
    exit 1
    ;;
esac
