# Czar Release Checklist

_Updated at: 2026-10-09_

Use this checklist for each release. Crates go to crates.io by hand (`just publish-crates`); the Python packages go to PyPI from the Release workflow when a `v*` tag is pushed.

## Release Metadata

- Release version: `vX.Y.Z`
- Date (UTC): `YYYY-MM-DD`
- Release owner:
- Notes link:

## 1) Preflight

- [ ] Working tree is clean
- [ ] `just lint` passes
- [ ] `cargo test --workspace` passes
- [ ] `just release-check` passes (metadata, publish list, nothing private in any package or tracked file)
- [ ] `just quality-report` runs (build, lint, Docker image, unit, integration, frontend and smoke tests, docs, examples, licenses, perf). The release owner reads its summary in `.czar/reports/quality/<timestamp>/` before deciding, and compares it with the previous release's. Keep the bundle in the private repository `czarhq/quality-reports` (its README says how, and `compare.py` puts two runs side by side). It isn't published, because its logs show local paths and details of the machine
- [ ] `./scripts/embed-ui.sh && cargo publish --workspace --dry-run --locked --allow-dirty` passes: it packages every crate and builds each one from its package, as crates.io will (`--allow-dirty` only because of the embedded UI)

## First public release only

### The repository

The public repository starts with one commit holding the tree of the release, without the private history:

```bash
CZAR_PUBLIC_AUTHOR="Name <id+user@users.noreply.github.com>" scripts/public-snapshot.sh main /tmp/czar-public
cd /tmp/czar-public && ./scripts/release-check.py
git remote add origin git@github.com:czarhq/czar.git && git push -u origin main
```

The script only builds the directory; pushing is a separate, deliberate step. Then, in the repository settings:

- [ ] Security → Private vulnerability reporting: on (SECURITY.md points there)
- [ ] Actions → General → Workflow permissions: read repository contents only
- [ ] Pages → Source: GitHub Actions (the Docs workflow publishes the site)
- [ ] Environments: add `pypi` (the Release workflow publishes from it)

### PyPI

Publishing uses trusted publishing, so no token is stored anywhere. Once, on PyPI:

- [ ] `czar` (the project exists): Manage → Publishing → add a GitHub publisher: owner `czarhq`, repository `czar`, workflow `release.yml`, environment `pypi`
- [ ] `czar-cli` (new): Account → Publishing → add a pending publisher with the same values and project name `czar-cli`

## 2) Versioning

- [ ] `[workspace.package].version` in `Cargo.toml`, and `version` in `python-sdk/pyproject.toml` and `python-cli/pyproject.toml` (with the `czar-cli==` pins in the SDK's extras)
- [ ] Any release notes/changelog updated
- [ ] Version commit created

## 3) Publish the crates

```bash
just publish-crates
```

It embeds the web UI, then publishes each crate in dependency order: `czar-core`, `czar-macros`, `czar-store-file`, `czar-store-sqlite`, `czar-store-postgres`, `czar-store-d1`, `czar-executors`, `czar-gcp`, `czar-store`, `czar-web`, `czar-worker`, `czar-tui`, `czar`, `czar-cli`. Crates already published at this version are skipped, so it can be run again after a failure.

crates.io lets one account publish only a few new crates at once, then about one every 10 minutes; the script waits when it hits that limit. A first release of all 14 crates takes about an hour and a half; later releases of existing crates are not limited this way.

## 4) Publish the Python packages

- [ ] Optional: run the Release workflow by hand (Actions → Release → Run workflow). It builds the `czar-cli` wheels for Linux, macOS and Windows and the `czar` SDK without publishing them.
- [ ] Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`. The Release workflow builds everything again and publishes to PyPI.

## 5) Post-Publish Verification

In a clean environment (a fresh container):

- [ ] `cargo install czar-cli --locked` succeeds; `czar --help` and `czar serve --help` run
- [ ] `cargo install czar-tui --locked` and `cargo install czar-worker --locked` succeed
- [ ] `uv tool install czar-cli` succeeds and `czar --help` runs; in a new uv project, `uv add czar && uv add --dev "czar[cli]"` then `uv run czar --help` works
- [ ] The README quick start works as written

## 6) Wrap-Up

- [ ] Create GitHub release notes (if wanted)
- [ ] Record any follow-up issues
