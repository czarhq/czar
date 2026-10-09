#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/deps-common.sh"

command_name="${1:-}"
require_cmd pnpm

run_install() {
  local dir status=0 install_log
  for dir in $(js_project_dirs); do
    if ! is_summary; then
      echo "==> ${dir#$ROOT_DIR/}"
    fi
    if is_verbose; then
      if ! pnpm install --dir "$dir" --frozen-lockfile; then
        status=1
      fi
    else
      install_log="$(mktemp)"
      if ! pnpm install --dir "$dir" --frozen-lockfile --reporter=silent >"$install_log" 2>&1; then
        status=1
        cat "$install_log"
      elif ! is_summary; then
        echo "ok: lockfile satisfied"
      fi
      rm -f "$install_log"
    fi
    if ! is_summary; then
      echo
    fi
  done
  return "$status"
}

run_outdated() {
  local dir status=0 rc=0
  for dir in $(js_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    pnpm outdated --dir "$dir" || rc=$?
    if [[ "$rc" -gt 1 ]]; then
      status=1
    fi
    rc=0
    echo
  done
  return "$status"
}

run_licenses() {
  local dir status=0 report_file project_count=0 package_count=0 skipped_count=0
  run_install || status=1
  for dir in $(js_project_dirs); do
    if ! is_summary; then
      echo "==> ${dir#$ROOT_DIR/}"
    fi
    if ! python3 - "$dir/package.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
raise SystemExit(0 if data.get("dependencies") else 1)
PY
    then
      skipped_count=$((skipped_count + 1))
      if ! is_summary; then
        echo "No production dependencies to license-scan."
        echo
      fi
      continue
    fi
    report_file="$(mktemp)"
    if ! pnpm dlx license-checker-rseidelsohn \
      --start "$dir" \
      --production \
      --onlyAllow "$CZAR_JS_LICENSE_ALLOW_ONLY" \
      --json \
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
data = json.load(open(sys.argv[1], encoding="utf-8"))
print(len(data))
PY
    )
    if is_summary; then
      total_packages="${total_packages:-0}"
      total_packages=$((total_packages + package_count))
    elif is_verbose; then
      python3 - "$report_file" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
for name in sorted(data):
    package = data[name]
    print(f"{name} :: {package.get('licenses', '?')}")
PY
    else
      python3 - "$report_file" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
licenses = sorted({(pkg.get("licenses") or "?") for pkg in data.values()})
print(f"ok: {len(data)} production package(s); licenses: {', '.join(licenses) if licenses else 'none'}")
PY
    fi
    rm -f "$report_file"
    if ! is_summary; then
      echo
    fi
  done
  if is_summary && [[ "$status" -eq 0 ]]; then
    echo "js projects=${project_count} packages=${total_packages:-0} skipped=${skipped_count}"
  fi
  return "$status"
}

run_freshness() {
  run_install
}

# pnpm enforces minimumReleaseAge from each pnpm-workspace.yaml.
run_update() {
  local dir status=0
  for dir in $(js_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    if ! pnpm update --dir "$dir"; then
      status=1
    fi
    echo
  done
  return "$status"
}

run_vulns() {
  local dir status=0
  for dir in $(js_project_dirs); do
    echo "==> ${dir#$ROOT_DIR/}"
    if ! pnpm audit --dir "$dir" --audit-level low; then
      status=1
    fi
    echo
  done
  return "$status"
}

case "$command_name" in
  install) run_install ;;
  outdated) run_outdated ;;
  licenses) run_licenses ;;
  freshness) run_freshness ;;
  vulns) run_vulns ;;
  update) run_update ;;
  *)
    echo "Usage: $0 {install|outdated|licenses|freshness|vulns|update}" >&2
    exit 1
    ;;
esac
