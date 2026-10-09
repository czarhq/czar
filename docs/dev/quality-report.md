# Quality Report

_Updated at: 2026-05-22_

Use `just quality-report` to run the main repo health checks and write a timestamped report bundle under `.czar/reports/quality/`.

## Command

```bash
just quality-report
```

The command uses the Python renderer in [`scripts/quality_report.py`](../../scripts/quality_report.py) and Jinja templates in [`scripts/templates/quality_report/`](../../scripts/templates/quality_report/). Its Python dependencies are managed by [`scripts/pyproject.toml`](../../scripts/pyproject.toml), not inline `uv --with ...` flags. It prints the destination bundle path first, then shows a colored step plan and per-step progress with durations, current/minimum available memory, and log-file paths while the checks run. All executed checks are routed through named `just` targets such as `just quality-build`, `just quality-docker`, `just quality-smoke-dev-stack`, `just quality-docs`, and `just quality-examples-system`.

By default, the report chooses a conservative `CARGO_BUILD_JOBS` value for each step from available physical memory reported by the host OS and the logical CPU count. Linux uses `/proc/meminfo` first, macOS uses `vm_stat`, Windows uses `GlobalMemoryStatusEx`, and other POSIX-like hosts fall back to `sysconf` when available. If available memory cannot be detected, the report uses a conservative CPU-capped fallback. Set `CARGO_BUILD_JOBS` yourself to override the adaptive value.

## Output Layout

Each run creates:

```text
.czar/reports/quality/<timestamp>/
```

and updates:

```text
.czar/reports/quality/latest
```

The bundle contains:

- `overview.md`: concise executive summary with links to every detail page
- `environment.md`: git/toolchain/environment snapshot
- `build.md`: build, lint, and Docker image smoke results
- `tests.md`: Rust unit, Rust integration, frontend, client smoke, and dev-stack smoke test status
- `docs.md`: docs/link/example-doc validation status
- `examples.md`: runnable example and validation status
- `dependencies.md`: license, freshness, vulnerability, and outdated summaries
- `benchmark.md`: latest recorded benchmark baseline summary
- `logs/*.log`: raw command output for each check
- `artifacts/*.json`: compact machine-readable artifacts for automation

## What It Covers

The executive summary rolls up:

- build
- lint
- Docker image smoke
- Rust unit tests
- Rust integration tests
- frontend tests
- client smoke / e2e tests
- Dockerized dev-stack smoke test
- docs checks
- example validation
- runnable example system checks
- dependency licenses
- dependency freshness
- dependency vulnerabilities
- outdated dependency reporting
- latest recorded benchmark baseline summary

## Notes

- The report is intentionally timestamped and gitignored.
- The markdown files are for humans; the JSON artifacts are compact and machine-oriented.
- `just quality-report` does not accept ad hoc positional overrides. The output root and example timeout are repo policy, not user-facing flags.
- Cargo build parallelism is adaptive when `CARGO_BUILD_JOBS` is unset. Each raw step log records the value used for that step.
- Each step records memory telemetry in its raw log and in `artifacts/checks.json`: start available memory, minimum available memory seen during the step, end available memory, and the OS source used for the reading. The overview includes the same values for quick comparison.
- The benchmark section checks the latest recorded baseline through `just benchmark-summary`; it does not run the full perf suite as part of `just quality-report`.
- Docs status currently covers checked links and checked snippet/example references.
- The report only states what actually ran. For example, it reports host-target builds today rather than pretending to validate all installed targets.
- The smoke suite is folded into `tests.md` and includes both the client smoke path (`just quality-smoke`) and the Dockerized dev-stack path (`just quality-smoke-dev-stack`).
- Docker coverage currently targets the standalone `crates/czar-cli/Dockerfile` and `crates/czar-web-react/Dockerfile` images, and checks that they both build and run.
