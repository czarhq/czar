#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

# Publish order: foundational libraries first, binaries last.
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

# For an unpublished multi-crate workspace, `cargo publish --dry-run` can only
# reach crates whose internal dependencies are already visible in the crates.io
# index. After publishing a stage, pass the next crate names through
# CZAR_PUBLISH_DRY_RUN_PACKAGES to check them before uploading.
FIRST_PUBLISH_DRY_RUN_PACKAGES=(
  czar-core
  czar-macros
  czar-tui
)

allow_dirty_args=()
if [[ "${CZAR_ALLOW_DIRTY:-0}" == "1" ]]; then
  allow_dirty_args+=(--allow-dirty)
fi

package_verify_args=()
# Before first publish, cross-crate dev-deps are not resolvable from crates.io.
if [[ "${CZAR_PACKAGE_VERIFY:-0}" != "1" ]]; then
  package_verify_args+=(--no-verify)
fi

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

echo "==> Packaging manifest checks (cargo package --list)"
for pkg in "${PACKAGES[@]}"; do
  echo "  - cargo package --list -p ${pkg}"
  web_args=$(web_dirty_args "${pkg}")
  cargo package --list -p "${pkg}" --locked "${allow_dirty_args[@]}" ${web_args} >/dev/null
done

if [[ "${CZAR_PACKAGE_TARBALL:-0}" == "1" ]]; then
  echo "==> Tarball creation checks (cargo package)"
  if [[ -n "${CZAR_PACKAGE_TARBALL_PACKAGES:-}" ]]; then
    read -r -a tarball_packages <<<"${CZAR_PACKAGE_TARBALL_PACKAGES}"
  else
    tarball_packages=("${FIRST_PUBLISH_DRY_RUN_PACKAGES[@]}")
  fi

  for pkg in "${tarball_packages[@]}"; do
    echo "  - cargo package -p ${pkg}"
    web_args=$(web_dirty_args "${pkg}")
    cargo package -p "${pkg}" --locked \
      "${package_verify_args[@]}" "${allow_dirty_args[@]}" ${web_args}
  done
fi

if [[ "${CZAR_SKIP_PUBLISH_DRY_RUN:-0}" == "1" ]]; then
  echo "Skipping publish dry-run (CZAR_SKIP_PUBLISH_DRY_RUN=1)."
  exit 0
fi

if [[ "${CZAR_RUN_PUBLISH_DRY_RUN:-0}" != "1" ]]; then
  echo "Skipping publish dry-run by default."
  echo "Set CZAR_RUN_PUBLISH_DRY_RUN=1 to enable cargo publish --dry-run checks."
  exit 0
fi

publish_verify_args=()
if [[ "${CZAR_PUBLISH_NO_VERIFY:-0}" == "1" ]]; then
  publish_verify_args+=(--no-verify)
fi

echo "==> Publish dry-run checks"
if [[ -n "${CZAR_PUBLISH_DRY_RUN_PACKAGES:-}" ]]; then
  read -r -a dry_run_packages <<<"${CZAR_PUBLISH_DRY_RUN_PACKAGES}"
else
  dry_run_packages=("${FIRST_PUBLISH_DRY_RUN_PACKAGES[@]}")
fi

for pkg in "${dry_run_packages[@]}"; do
  echo "  - cargo publish -p ${pkg} --dry-run"
  web_args=$(web_dirty_args "${pkg}")
  cargo publish -p "${pkg}" --dry-run --locked \
    "${publish_verify_args[@]}" "${allow_dirty_args[@]}" ${web_args}
done
