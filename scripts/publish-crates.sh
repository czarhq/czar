#!/usr/bin/env bash
# Publish every crate to crates.io, dependencies first (the order of
# publish-prep.sh). A crate already on crates.io at this version is skipped,
# so the script can simply be run again after a failure.
#
# crates.io limits how many new crates one account may publish: a few at
# once, then about one every 10 minutes. On that error the script waits and
# tries the same crate again, so a first release of every crate takes a while.
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

PACKAGES=(
  czar-core
  czar-macros
  czar-store-file
  czar-store-sqlite
  czar-store-postgres
  czar-store-d1
  czar-executors
  czar-gcp
  czar-store
  czar-web
  czar-worker
  czar-tui
  czar
  czar-cli
)
WAIT_SECONDS="${CZAR_PUBLISH_WAIT_SECONDS:-600}"

version=$(cargo metadata --no-deps --format-version 1 --offline |
  python3 -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "czar-core"))')

# czar-web packages the embedded web UI (crates/czar-web/ui), which git
# ignores, so cargo counts it as uncommitted. Allow that for czar-web only, and
# only when nothing else in the crate is uncommitted.
web_dirty_args() {
  if [[ "$1" == "czar-web" ]]; then
    if [[ -n "$(git status --porcelain -- crates/czar-web)" ]]; then
      echo "crates/czar-web has uncommitted changes; commit them first" >&2
      return 1
    fi
    echo "--allow-dirty"
  fi
}

echo "==> Embedding the web UI into czar-web"
./scripts/embed-ui.sh

for pkg in "${PACKAGES[@]}"; do
  if curl -fsS -o /dev/null -A "czar-publish-script" "https://crates.io/api/v1/crates/${pkg}/${version}" 2>/dev/null; then
    echo "==> ${pkg} ${version} is already on crates.io"
    continue
  fi
  while true; do
    echo "==> Publishing ${pkg} ${version}"
    web_args=$(web_dirty_args "${pkg}")
    if output=$(cargo publish -p "${pkg}" --locked ${web_args} 2>&1); then
      echo "${output}" | tail -n 3
      break
    fi
    echo "${output}" | tail -n 15
    if grep -qiE "429|too many|try again after" <<<"${output}"; then
      echo "    crates.io rate limit: waiting ${WAIT_SECONDS}s, then trying ${pkg} again"
      sleep "${WAIT_SECONDS}"
    else
      exit 1
    fi
  done
done
echo "All ${#PACKAGES[@]} crates are on crates.io at ${version}."
