# Smoke Tests

_Updated at: 2026-05-22_

Use the smoke suite when you want one small end-to-end check across the local entrypoints and the Dockerized dev stack:

```bash
just smoke-test
```

This runs:

- `just smoke-test-clients` Convenience wrapper for the three non-Docker client smoke tests below.
- `just smoke-test-cli` Runs the `simple_args` workflow through the CLI against a fresh local SQLite database and waits for it to complete.
- `just smoke-test-web` Starts a local SQLite-backed API plus Vite frontend, drives the React UI with Playwright, clicks the workflow and `Run` button, waits for the run to reach `success`, and fails on any browser console warning, browser console error, page error, or failed request.
- `just smoke-test-tui` Starts a local SQLite-backed API, launches `czar-tui` in a PTY, triggers the selected workflow with the real `t` keybinding, waits for the rendered terminal view to show a successful run, and fails if the client exits with an error.
- `just smoke-test-dev-stack` Boots the same Docker Compose stack used by `just up`, but under an isolated project name and random host ports. It waits for the frontend HTML to load, verifies the frontend `/api` proxy can list workflows, triggers `simple_args` through the API, and waits for the run to finish successfully.

Alias:

```bash
just e2e-test
```

## Docker Smoke

To verify the standalone Docker images actually build and run:

```bash
just docker-smoke
```

This currently checks:

- `crates/czar-cli/Dockerfile` Builds the CLI image and runs `czar --help` inside the container.
- `crates/czar-web-react/Dockerfile` Builds the web image, starts the nginx container, and waits for an HTTP 200 HTML response.

This is intentionally separate from `just smoke-test-dev-stack`, which exercises the Docker Compose dev stack behind `just up`.

## Browser Setup

The web smoke test uses Playwright Chromium. Install it once with:

```bash
just smoke-test-install
```

## Local Stack

The smoke suite is deterministic:

- workflow source: `examples/workflows/simple_args`
- database: fresh SQLite files under `.czar/smoke/`
- API server: local `czar serve`
- web UI: local Vite dev server
- Docker dev stack: `crates/czar-cli/docker-compose.dev.yml` with isolated project name and random host ports

For manual debugging, you can start the stack pieces separately:

```bash
just smoke-stack-api-server
just smoke-web-frontend-server
```

The web smoke test uses `http://127.0.0.1:4010` for the API and `http://127.0.0.1:4176` for the frontend by default.
