set shell := ["sh", "-c"]

# The docs tool (https://github.com/gajop/in-ctrl); IN_CTRL=path/to/in-ctrl/bin/in-ctrl.mjs uses a local copy.
in_ctrl := env_var_or_default("IN_CTRL", "npx --yes in-ctrl@0.1.0")

default:
    @just --list

# Build all Rust crates in workspace.
[group('build')]
build:
    cargo build --workspace

# Build optimized binaries.
[group('build')]
build-release:
    cargo build --workspace --release

# Install the local `czar` CLI binary (SQLite + local executors).
[group('build')]
install:
    cargo install --path crates/czar-cli --no-default-features --features sqlite,local-executors --bin czar --force

# Install the official supply-chain tools used by the repo.
[group('deps')]
deps-install-tools:
    cargo install --locked cargo-deny cargo-outdated

# Check outdated dependencies with the ecosystem-native tools.
[group('deps')]
outdated: outdated-core outdated-examples outdated-frontend

# Alias for the multi-ecosystem outdated report.
[group('deps')]
deps-outdated: outdated

# Check all dependency licenses with the ecosystem-native tools.
[group('deps')]
licenses:
    @bash ./scripts/deps-licenses.sh

# Check dependency licenses with package-by-package detail.
[group('deps')]
licenses-verbose:
    @CZAR_DEPS_VERBOSE=1 bash ./scripts/deps-licenses.sh

# Alias for the dependency license policy check.
[group('deps')]
deps-licenses: licenses

# Alias for the verbose dependency license check.
[group('deps')]
deps-licenses-verbose: licenses-verbose

# Check the release-age policy for ecosystems with native support.
[group('deps')]
freshness:
    python3 ./scripts/deps-age-check.py
    bash ./scripts/deps-python.sh freshness
    bash ./scripts/deps-js.sh freshness

# Alias for the release-age gate.
[group('deps')]
deps-freshness: freshness

# Scan dependencies for known vulnerabilities.
[group('deps')]
vulns:
    bash ./scripts/deps-cargo.sh vulns
    bash ./scripts/deps-python.sh vulns
    bash ./scripts/deps-js.sh vulns

# Alias for the vulnerability scan.
[group('deps')]
deps-vulns: vulns

# Run the full supply-chain policy suite.
[group('deps')]
deps-check:
    just outdated
    just licenses
    just freshness
    just vulns

# Refresh every lockfile (Rust, Python, JS) to the newest releases that fit the
# manifest ranges and are at least CZAR_MIN_RELEASE_AGE_DAYS (default 7) old.
[group('deps')]
deps-update:
    bash ./scripts/deps-cargo.sh update
    bash ./scripts/deps-python.sh update
    bash ./scripts/deps-js.sh update
    just deps-majors

# Move one Rust crate to its newest release that is at least
# CZAR_MIN_RELEASE_AGE_DAYS (default 7) old, e.g. for a security fix.
[group('deps')]
deps-update-crate crate:
    bash ./scripts/deps-cargo.sh update-crate {{crate}}

# List aged upgrades the manifest ranges block (need a manual manifest bump).
[group('deps')]
deps-majors:
    bash ./scripts/deps-cargo.sh majors
    bash ./scripts/deps-python.sh outdated
    bash ./scripts/deps-js.sh outdated

# Check outdated dependencies for the main Rust workspace crates.
[group('deps')]
outdated-core:
    bash ./scripts/deps-cargo.sh outdated

# Check outdated dependencies for official example Python projects.
[group('deps')]
outdated-examples:
    bash ./scripts/deps-python.sh outdated

# Check outdated dependencies for repo-owned JS projects.
[group('deps')]
outdated-frontend:
    bash ./scripts/deps-js.sh outdated

# Install JS dependencies for the repo-owned frontend and Cloudflare projects.
[group('deps')]
frontend-install:
    bash ./scripts/deps-js.sh install

# Fail fast with a clear message when the React toolchain is not installed.
[group('deps')]
frontend-check:
    cd crates/czar-web-react && [ -x node_modules/.bin/vite ] || (echo "Frontend dependencies are missing. Run 'just frontend-install' first." && exit 1)

# Start the full Dockerized dev stack (Postgres + API + frontend with dev containers).
[group('dev')]
up:
    ./scripts/dev-up.sh

# Alias for older docs/scripts.
[group('dev')]
example-up: up

# Start the SQLite-backed local stack without Docker.
[group('dev')]
up-sqlite:
    ./scripts/dev-up-sqlite.sh

# Start the Cloudflare Worker + container target locally with a Wrangler-backed D1 harness.
[group('dev')]
up-cloudflare-full:
    ./scripts/cloudflare-full-local.sh up

# Start the direct Cloudflare app container locally against the Wrangler-backed D1 harness.
[group('dev')]
up-cloudflare-full-direct:
    ./scripts/cloudflare-full-local.sh up-direct

# Stop the Dockerized dev stack.
[group('dev')]
down:
    ./scripts/dev-down.sh

# Kill any czar server process listening on port 4000 (for `just up-sqlite` run in background).
[group('dev')]
down-sqlite:
    ./scripts/dev-down-sqlite.sh

# Stop the local Cloudflare Worker + container target and its Wrangler D1 harness.
[group('dev')]
down-cloudflare-full:
    ./scripts/cloudflare-full-local.sh down

# Print the current local Cloudflare logs from the managed state dir.
[group('dev')]
cloudflare-full-logs:
    ./scripts/cloudflare-full-local.sh logs

# Create + trigger an example workflow by folder name (expects examples/workflows/<name>/<name>.yaml).
[group('examples')]
example name:
    bash ./scripts/run-example.sh {{name}}

[group('examples')]
example-build name profile='debug':
    bash ./scripts/build-example.sh {{name}} {{profile}}

[group('examples')]
example-all:
    #!/usr/bin/env bash
    set -euo pipefail
    for name in $(python3 ./scripts/examples.py list); do
        echo "==> $name"
        just example "$name"
    done

# Start the scheduler + API locally (DB-backed).
[group('dev')]
run-scheduler:
    cargo run -p czar-cli --bin czar -- serve

# Start only the standalone web API binary locally.
[group('dev')]
run-web:
    cargo run -p czar-web -- --addr 127.0.0.1:4000

# Build the React frontend.
[group('dev')]
build-frontend: frontend-check
    cd crates/czar-web-react && pnpm run build

# Start React UI in dev mode (proxies to czar-web on :4000, port 5175).
[group('dev')]
dev-frontend: frontend-check
    cd crates/czar-web-react && pnpm run dev

# Connect the Ratatui TUI to an already-running server (e.g. started by `just up-sqlite`).
[group('dev')]
czar-tui base_url='http://127.0.0.1:4000':
    cargo run -p czar-tui -- --base-url {{base_url}}

# Start a SQLite-backed scheduler + web API, then launch the Ratatui TUI (self-contained).
[group('dev')]
up-czar-tui:
    ./scripts/up-czar-tui.sh

# Run the czar CLI with arbitrary arguments (e.g. `just czar trigger my-workflow`).
[group('dev')]
czar *args:
    cargo run -p czar-cli --bin czar -- {{args}}

# Show persisted runs/status (DB-backed).
[group('dev')]
status:
    cargo run -p czar-cli --bin czar -- status

# Run all tests
[group('test')]
test: frontend-check
    @echo "Running all tests..."
    cargo test --workspace
    cd crates/czar-web-react && pnpm test

# Run tests with output
[group('test')]
test-verbose:
    @echo "Running all tests with output..."
    cargo test --workspace -- --nocapture

# Run tests for SQLite backend
[group('test')]
test-sqlite:
    @echo "Running SQLite tests..."
    cargo test --workspace --no-default-features --features sqlite,process

# Run tests for Postgres backend
[group('test')]
test-postgres:
    @echo "Running Postgres tests..."
    cargo test --workspace --features postgres

# Run the ignored toxiproxy-backed scheduler resilience tests against local Postgres.
[group('test')]
test-postgres-toxiproxy:
    @echo "Running toxiproxy-backed scheduler tests..."
    cargo test -p czar-core --test scheduler_processing_behavior_test toxiproxy -- --ignored --nocapture

# Run a specific test by name
[group('test')]
test-one name:
    @echo "Running test: {{name}}"
    cargo test --workspace {{name}} -- --nocapture --exact

# Run the Cloudflare D1 emulator-backed integration check through wrangler dev.
[group('test')]
test-cloudflare-d1-local:
    cd crates/czar-store-d1/wrangler-harness && pnpm install --frozen-lockfile
    cargo test -p czar-store-d1 --test wrangler_dev -- --ignored --nocapture

# Run the full Cloudflare Worker + container smoke test through wrangler dev.
[group('test')]
test-cloudflare-full-local:
    ./scripts/cloudflare-full-local.sh test

# Run the direct app + D1 smoke test without the Worker/Containers edge layer.
[group('test')]
test-cloudflare-full-direct-local:
    ./scripts/cloudflare-full-local.sh test-direct

# Install the Chromium browser used by the web smoke test.
[group('test')]
smoke-test-install: frontend-check
    cd crates/czar-web-react && pnpm exec playwright install chromium

# Install dependencies for the Lighthouse frontend performance test.
[group('test')]
perf-frontend-install: frontend-check
    cd crates/czar-web-react && pnpm install --frozen-lockfile
    cd crates/czar-web-react && pnpm exec playwright install chromium

# Build and serve the production frontend for the Lighthouse perf test.
[group('test')]
perf-web-frontend-server api_url='http://127.0.0.1:4012' port='4178': frontend-check
    #!/usr/bin/env sh
    set -e
    cd crates/czar-web-react
    CZAR_WEB_API_PROXY_TARGET={{api_url}} pnpm run build
    CZAR_WEB_API_PROXY_TARGET={{api_url}} pnpm exec vite preview --host 127.0.0.1 --port {{port}} --strictPort

# Run the Lighthouse frontend performance audit against the smoke stack.
[group('test')]
perf-frontend: frontend-check
    cd crates/czar-web-react && env -u NO_COLOR -u FORCE_COLOR pnpm run test:perf

# Run the CDP graph rendering benchmark for large workflows (deep_graph + wide_graph, 1k nodes each).
[group('test')]
graph-bench: frontend-check
    cd crates/czar-web-react && env -u NO_COLOR -u FORCE_COLOR pnpm run test:graph-bench

# Start the deterministic local SQLite smoke-test API stack.
[group('test')]
smoke-stack-api-server addr='127.0.0.1:4010' db_dir='.czar/smoke/api' workflows_dir='examples/workflows/simple_args':
    cd scripts && uv run python ./smoke_stack.py api-server --addr {{addr}} --db-dir {{db_dir}} --workflows-dir {{workflows_dir}}

# Build and serve the production frontend against the local smoke-test API stack.
[group('test')]
smoke-web-frontend-server api_url='http://127.0.0.1:4010' port='4176': frontend-check
    #!/usr/bin/env sh
    set -e
    cd crates/czar-web-react
    env -u NO_COLOR -u FORCE_COLOR CZAR_WEB_API_PROXY_TARGET={{api_url}} pnpm run build
    env -u NO_COLOR -u FORCE_COLOR CZAR_WEB_API_PROXY_TARGET={{api_url}} pnpm exec vite preview --host 127.0.0.1 --port {{port}} --strictPort

# Run the CLI smoke test against a small local SQLite workflow.
[group('test')]
smoke-test-cli:
    cd scripts && uv run python ./smoke_test.py cli

# Run the Playwright-driven web smoke test.
[group('test')]
smoke-test-web: frontend-check
    cd crates/czar-web-react && env -u NO_COLOR -u FORCE_COLOR pnpm exec playwright test

# Run the PTY-driven Ratatui smoke test.
[group('test')]
smoke-test-tui:
    cd scripts && uv run python ./smoke_test.py tui

# Run the Dockerized dev stack behind `just up` and validate it can trigger a real workflow.
[group('test')]
smoke-test-dev-stack:
    cd scripts && uv run python ./docker_smoke.py dev-stack

# Build and run the standalone CLI and web Docker images.
[group('test')]
docker-smoke:
    cd scripts && uv run python ./docker_smoke.py standalone

# Build the czar-worker Docker image and run the end-to-end worker test.
[group('test')]
test-worker-e2e:
    cd tests/worker-e2e && uv run pytest test_worker_e2e.py -v

# Run the local client smoke suite without the Dockerized dev stack.
[group('test')]
smoke-test-clients:
    just smoke-test-cli
    just smoke-test-web
    just smoke-test-tui

# Run the first end-to-end smoke suite across CLI, web, and TUI.
[group('test')]
smoke-test:
    just smoke-test-clients
    just smoke-test-dev-stack

# Run all end-to-end tests: smoke suite + worker e2e.
[group('test')]
e2e-test:
    just smoke-test
    just test-worker-e2e

# Run tests and generate coverage report (requires cargo-llvm-cov)
[group('test')]
test-coverage:
    @echo "Generating test coverage..."
    cargo llvm-cov --workspace --html --output-dir coverage

# Run Postgres backend coverage for czar-store (requires running Postgres and cargo-llvm-cov).
[group('test')]
test-coverage-postgres:
    @echo "Generating Postgres coverage for czar-store..."
    cargo llvm-cov -p czar-store --features postgres --test postgres_backend_test --html --output-dir coverage/postgres -- --nocapture

# Run all linting checks (does not check doc freshness; see lint-all).
[group('lint')]
lint: frontend-check
    ./scripts/check-workspace-lints.py
    cargo check --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    cd crates/czar-web-react && pnpm run typecheck
    cd crates/czar-web-react && pnpm run build
    ./scripts/check-loc.py
    ./scripts/check-doc-examples.py
    python3 ./scripts/examples.py validate --results .czar/reports/examples-validate.json --skip-docs-validation

[group('lint')]
scripts-lint:
    cd scripts && uv run --group dev ruff check .
    cd scripts && uv run --group dev pyright

# Check what a release would publish: crate metadata, the publish list, and
# that no package or tracked file holds anything private.
[group('publish')]
release-check:
    ./scripts/release-check.py

# Validate crate packaging + publish dry-run in publish order.
[group('publish')]
publish-prep:
    ./scripts/publish-prep.sh

# Publish every crate to crates.io (re-runnable; waits out rate limits)
[group('publish')]
publish-crates:
    ./scripts/publish-crates.sh

# Local variant for uncommitted changes.
[group('publish')]
publish-prep-dirty:
    CZAR_ALLOW_DIRTY=1 ./scripts/publish-prep.sh

# Local/offline-ish variant when only packaging should be checked.
[group('publish')]
publish-prep-package-only:
    CZAR_ALLOW_DIRTY=1 CZAR_SKIP_PUBLISH_DRY_RUN=1 ./scripts/publish-prep.sh

# Full flow (includes cargo publish --dry-run; useful after bootstrap publish order exists).
[group('publish')]
publish-prep-full:
    CZAR_ALLOW_DIRTY=1 CZAR_RUN_PUBLISH_DRY_RUN=1 ./scripts/publish-prep.sh

# Run lint checks (cached, fast on re-run). For clippy auto-fix run 'just clippy-fix'.
[group('lint')]
lint-fix:
    cargo clippy --workspace --all-targets
    ./scripts/check-loc.py

# Run all linting checks including doc freshness (use for deep quality evaluation).
[group('lint')]
lint-all: lint docs-check

# Apply clippy auto-fixes (always re-checks; use sparingly).
[group('lint')]
clippy-fix:
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged

# Check all markdown files for broken relative links.
[group('lint')]
check-links:
    ./scripts/check-links.py

# Render the diagrams under docs/**/diagrams/ that changed (in-ctrl).
[group('lint')]
docs-diagrams:
    {{in_ctrl}} render

# The docs site, live, as GitHub Pages will show it.
[group('lint')]
docs-site:
    {{in_ctrl}} site dev

# Check the docs: links, examples, crate pages and rendered diagrams.
[group('lint')]
docs-check:
    ./scripts/check-links.py
    ./scripts/check-doc-examples.py
    ./scripts/check-docs.py
    {{in_ctrl}} check

# List available workflow examples.
[group('examples')]
examples-list:
    @python3 ./scripts/examples.py list

# List all examples with their execution policy.
[group('examples')]
examples-list-all:
    @python3 ./scripts/examples.py list --all --details

# Validate all examples; run only those marked runnable.
[group('examples')]
examples-check-all timeout_s='240':
    python3 ./scripts/examples.py check --timeout {{timeout_s}} --results .czar/reports/examples-check.json

# Validate all examples without running the runnable workflows.
[group('examples')]
examples-validate:
    python3 ./scripts/examples.py validate --results .czar/reports/examples-validate.json --skip-docs-validation

[group('quality')]
quality-build:
    cargo build --workspace

[group('quality')]
quality-lint: frontend-check
    ./scripts/check-workspace-lints.py
    cargo check --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    cd crates/czar-web-react && pnpm run typecheck
    cd crates/czar-web-react && pnpm run build
    ./scripts/check-loc.py

[group('quality')]
quality-test-rust-unit:
    cargo test --workspace --lib

[group('quality')]
quality-test-rust-integration:
    cargo test --workspace --tests

[group('quality')]
quality-test-frontend: frontend-check
    cd crates/czar-web-react && pnpm test

[group('quality')]
quality-smoke:
    just smoke-test-clients

quality-smoke-dev-stack:
    just smoke-test-dev-stack

quality-smoke-all:
    just smoke-test

[group('quality')]
quality-docker:
    just docker-smoke

[group('quality')]
quality-docs:
    python3 ./scripts/check-links.py
    python3 ./scripts/check-doc-examples.py
    {{in_ctrl}} check

[group('quality')]
quality-examples-validate:
    python3 ./scripts/examples.py validate --results "${CZAR_QUALITY_EXAMPLES_VALIDATE_RESULTS:-.czar/reports/examples-validate.json}" --skip-docs-validation

[group('quality')]
quality-examples-system:
    python3 ./scripts/examples.py check --timeout "${CZAR_QUALITY_EXAMPLES_TIMEOUT:-240}" --results "${CZAR_QUALITY_EXAMPLES_RESULTS:-.czar/reports/examples-check.json}"

[group('quality')]
benchmark-summary:
    python3 ./scripts/benchmark_summary.py

# Run quality checks and emit a timestamped report bundle under .czar/reports/quality/<timestamp>/.
[group('quality')]
quality-report:
    @cd scripts && uv run python ./quality_report.py
