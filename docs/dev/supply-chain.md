# Supply-Chain Checks

_Updated at: 2026-09-29_

Czar exposes dependency hygiene through explicit `just` commands and standard ecosystem tools.

## Commands

Run these from the repo root:

```bash
just outdated
just licenses
just licenses-verbose
just freshness
just vulns
just deps-check
just deps-update
just deps-majors
```

What they use:

- Rust outdated: `cargo outdated`
- Rust licenses and advisories: `cargo deny`
- Python outdated and release-age gating: `uv`
- Python vulnerabilities: `pip-audit` via `uvx`
- Python licenses: `pip-licenses` via `uvx`
- JS outdated, install policy, and advisories: `pnpm`
- JS licenses: `license-checker-rseidelsohn` via `pnpm dlx`

`just deps-check` runs all four checks in sequence.

## Updating Dependencies

`just deps-update` refreshes every lockfile to the newest releases that fit the manifest ranges and are at least 7 days old:

- Rust: `cargo generate-lockfile --publish-time <cutoff>` for the workspace and `examples/workflows/polyglot/rust_tasks`. `--publish-time` is still an unstable cargo flag, so the script sets `RUSTC_BOOTSTRAP=1` for that one command. That lets the stable toolchain accept it; no nightly install is needed, and nothing is compiled with unstable features. If a future cargo renames or stabilizes the flag, this step fails loudly rather than skipping the gate.
- Python: `uv lock --upgrade` for every locked project; the age gate comes from each `pyproject.toml`.
- JS: `pnpm update` for each project; the age gate comes from `minimumReleaseAge`. This also raises the `^` floors in `package.json`.

Cargo doesn't apply the rule to a hand-run `cargo update` or `cargo add` yet: `.cargo/config.toml` sets `global-min-publish-age = "7 days"`, which cargo enforces only from 1.100 (2026-11-12). Until then `just freshness`, and CI on every pull request, run `scripts/deps-age-check.py`: it finds the crate versions that are new in any `Cargo.lock` compared with `origin/main` and fails if crates.io published one less than 7 days ago. `just deps-update-crate <crate>` moves one crate to its newest release that passes.

Some fixes can't wait 7 days, typically a security advisory whose fixed release is new. Then add an entry to [deps-age-exceptions.toml](../../deps-age-exceptions.toml): the crates it covers, the reason, and the date until which it applies (when the release is 7 days old; delete the entry after that). The check then lists those versions as allowed instead of failing. Python and JS have the same escape in their own settings: `exclude-newer-package` in a `pyproject.toml`, `minimumReleaseAgeExclude` in a `pnpm-workspace.yaml`.

It finishes by running `just deps-majors`, which lists upgrades the manifest ranges block (major bumps, pins, upper bounds). Those need a manifest edit and usually code changes, so they are never applied automatically. Rust majors come from `scripts/deps-cargo-majors.py`, which asks crates.io for the newest stable release published before the cutoff.

After `just deps-update`, run `just lint` and `just test` before committing.

`just licenses` is summary-oriented by default. It prints a single aggregated success line when the policy passes, and expands to real tool output only when there is a failure.

Use `just licenses-verbose` when you want the full package-by-package license listing even on success.

`just outdated` is intentionally report-oriented. It surfaces version drift, but it does not fail only because a newer package exists.

`just vulns` is vulnerability-focused. Real Rust security advisories still fail the command, while `cargo deny` unmaintained-crate advisories are surfaced as warnings unless and until there is a clean replacement path.

## Tool Bootstrap

Rust tools are installed explicitly:

```bash
just deps-install-tools
```

That installs:

- `cargo-deny`
- `cargo-outdated`

`uvx` is used for Python audit/license tools, so they do not need a separate global install.

`pnpm` is required for repo-owned JS projects.

## Python Age Gate

Python dependency checks use `uv`'s native `exclude-newer` support. This is mandatory policy in this repo.

Every locked Python project declares the gate in its `pyproject.toml`:

```toml
[tool.uv]
exclude-newer = "7 days"
```

uv records the span in `uv.lock` and applies it on every resolve. `just freshness` fails if a project is missing the setting or its lockfile is out of date. The wrappers do not pass `--exclude-newer` on the command line, because that overrides the relative span and rewrites the lockfile.

The repo wrappers require `uv >= 0.9.24`. If an older `uv` is on `PATH`, the command fails instead of silently dropping the age gate.

The Rust update and `deps-majors` steps compute their cutoff as midnight UTC, 7 days ago. Override it for a one-off run with `CZAR_MIN_RELEASE_AGE_DAYS=2` or `CZAR_UV_EXCLUDE_NEWER=2026-09-01T00:00:00Z`.

## JS Age Gate

Repo-owned JS projects use `pnpm` and set `minimumReleaseAge` in their local `pnpm-workspace.yaml`.

Current default:

- `minimumReleaseAge: 10080`

That is 7 days in minutes.

If a package must bypass that for a critical reason, add a narrowly scoped entry to the relevant project's `minimumReleaseAgeExclude`.

Python projects use the same idea through `tool.uv.exclude-newer-package`, with an explicit timestamp per package when a security fix must bypass the default repo-wide age gate.

Python vulnerability scanning exports the locked third-party requirement set from `uv.lock` and audits that export with `uvx pip-audit`. Local editable path dependencies, such as the in-repo `czar` SDK, are treated as first-party code and are not sent through PyPI vulnerability resolution.

## License Policy

The license checks are allowlist-based.

The practical policy is:

- permissive licenses are allowed
- copyleft or unknown licenses fail
- small metadata variations are tolerated when the underlying license is still clearly permissive

Current allowlist families include:

- MIT
- Apache-2.0
- BSD-2-Clause
- BSD-3-Clause
- ISC
- MPL-2.0
- Zlib
- Unlicense
- CDLA-Permissive-2.0
- similar permissive licenses already accepted by the configured tool invocations

Rust policy lives in [`deny.toml`](../../deny.toml).

## Scope

The dependency commands currently cover the repo-owned surfaces that ship or are actively maintained here:

- the main Rust workspace
- official Python example workflows under `examples/workflows`
- the `scripts` and `tests/worker-e2e/workflow` Python projects
- repo-owned JS projects under:
  - `crates/czar-web-react`
  - `crates/czar-store-d1/wrangler-harness`
  - `deploy/cloudflare/full`

They do not currently treat the comparison-framework projects under `docs/comparisons` as release-gated product dependencies.

## Notes

- These commands are intentionally not part of `just lint` because they depend on live registries and advisory services.
- `just deps-check` is the recommended pre-release dependency gate.
- Cargo has no stable release-age setting. `just deps-update` applies the gate when it resolves, but nothing stops a manual `cargo update` from pulling a newer crate, and `just freshness` does not check Rust.
