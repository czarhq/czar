# Releasing Czar Crates

_Updated at: 2026-05-22_

This guide covers Rust crate release prep and publish flow for Czar.

## Scope

- This is for crates.io publishing (not PyPI).
- It assumes the workspace metadata and publish-prep script in this repo are used.
- For per-release execution tracking, use [Release Checklist](release-checklist.md).

## Package Graph

Publish in dependency order:

1. `czar-core`
2. `czar-macros`
3. `czar-store-file`
4. `czar-store-sqlite`
5. `czar-store-postgres`
6. `czar-store-d1`
7. `czar-executors`
8. `czar-store`
9. `czar-web`
10. `czar-worker`
11. `czar-tui`
12. `czar`
13. `czar-cli`

## Local Preflight

Run these from the repo root:

```bash
just deps-check
just lint
cargo check --workspace
just publish-prep
```

Notes:

- `just deps-check` runs the network-backed dependency freshness, license, and vulnerability checks against the committed lockfiles.
- `just publish-prep` runs packaging manifest checks (`cargo package --list`) and skips publish dry-run by default.
- `just publish-prep-full` enables publish dry-runs (`cargo publish --dry-run`). For a first publish, it only checks crates that do not depend on unpublished internal crates yet.

## Version Bump

Set the release version in workspace metadata:

- Update `version` in `Cargo.toml` under `[workspace.package]`.
- Run `cargo check --workspace`.
- Commit the version change before publishing.

## First Publish (Bootstrap)

For the first crates.io publish of this crate family, crates.io dry-runs can only validate a crate once its internal dependencies are already visible in the crates.io index.

Before publishing the bootstrap roots, run:

```bash
just publish-prep-full
```

After each stage is visible in the index, dry-run the next stage explicitly:

```bash
CZAR_RUN_PUBLISH_DRY_RUN=1 CZAR_PUBLISH_DRY_RUN_PACKAGES="czar-store-file czar-store-sqlite czar-store-postgres czar-store-d1 czar-executors" ./scripts/publish-prep.sh
```

Then publish in dependency order:

```bash
cargo publish -p czar-core --locked
cargo publish -p czar-macros --locked
cargo publish -p czar-store-file --locked
cargo publish -p czar-store-sqlite --locked
cargo publish -p czar-store-postgres --locked
cargo publish -p czar-store-d1 --locked
cargo publish -p czar-executors --locked
cargo publish -p czar-store --locked
cargo publish -p czar-web --locked
cargo publish -p czar-worker --locked
cargo publish -p czar-tui --locked
cargo publish -p czar --locked
cargo publish -p czar-cli --locked
```

If crates.io index propagation lags, wait briefly and retry the next crate.

## Subsequent Releases

After initial bootstrap, run full dry-run checks:

```bash
just publish-prep-full
```

Then publish in the same order (typically without `--no-verify`):

```bash
cargo publish -p czar-core --locked
cargo publish -p czar-macros --locked
cargo publish -p czar-store-file --locked
cargo publish -p czar-store-sqlite --locked
cargo publish -p czar-store-postgres --locked
cargo publish -p czar-store-d1 --locked
cargo publish -p czar-executors --locked
cargo publish -p czar-store --locked
cargo publish -p czar-web --locked
cargo publish -p czar-worker --locked
cargo publish -p czar-tui --locked
cargo publish -p czar --locked
cargo publish -p czar-cli --locked
```

## CI Integration

- Baseline publish-prep runs in CI via `.github/workflows/ci.yml`.
- Full dry-run checks are gated by repository variable `CZAR_RUN_PUBLISH_DRY_RUN=1`.
