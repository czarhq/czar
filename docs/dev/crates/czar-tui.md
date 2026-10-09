# czar-tui

_Updated at: 2026-05-22_

Ratatui terminal UI for the Czar workflow orchestrator. Connects to a running `czar-web` API server and provides a keyboard-driven interface for browsing workflows, runs, task status, and logs.

## Features

- Sidebar with Workflows and Runs tabs (fuzzy search, Tab to switch)
- Detail panel: task list with inline log view (35%/65% split for runs)
- Full-screen DAG graph mode (`g` key) with arrow-key panning; layout cached via Kahn's BFS for O(n+e) performance
- Viewport clipping so large graphs (1000+ nodes) render efficiently
- Actions: trigger (`t`), cancel (`c`), pause/resume (`p`), refresh (`r`)

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/czar-tui/Cargo.toml) | TUI crate manifest declaring the `czar-tui` binary and terminal/UI/API dependencies. | 2026-03-05 | 8d5ae92 |
| [src/api.rs](../../../crates/czar-tui/src/api.rs) | HTTP API client and response decoding helpers used by the TUI to fetch workflows, runs, run detail, and control actions. | 2026-03-12 | cd21cc6 |
| [src/app/input.rs](../../../crates/czar-tui/src/app/input.rs) | Keyboard input handling and command dispatch for navigation, refresh, graph mode, and run/task actions. | 2026-03-12 | 3fc6aa2 |
| [src/app/mod.rs](../../../crates/czar-tui/src/app/mod.rs) | App state model, selection/filter helpers, and derived view calculations that drive terminal rendering. | 2026-03-12 | bc47990 |
| [src/app/worker.rs](../../../crates/czar-tui/src/app/worker.rs) | Background refresh worker that polls the API, streams updates into app state, and coordinates periodic refresh actions. | 2026-03-12 | 1e08ef2 |
| [src/dag.rs](../../../crates/czar-tui/src/dag.rs) | DAG layout engine that computes left-to-right node positions and edge segments with Kahn BFS for terminal graph rendering. | 2026-03-08 | 67c48dc |
| [src/main.rs](../../../crates/czar-tui/src/main.rs) | TUI entry point, terminal setup/teardown, event loop wiring, and top-level module composition. | 2026-03-12 | 6f7ab28 |
| [src/tests.rs](../../../crates/czar-tui/src/tests.rs) | App-level terminal UI tests covering state updates, fuzzy matching, progress math, and render smoke paths. | 2026-03-12 | bd501cc |
| [src/ui/detail.rs](../../../crates/czar-tui/src/ui/detail.rs) | Detail-pane rendering helpers for workflows, runs, task progress, logs, and run/task metadata. | 2026-03-12 | 8259eee |
| [src/ui/mod.rs](../../../crates/czar-tui/src/ui/mod.rs) | Top-level Ratatui rendering composition for sidebar, detail panes, graph mode, and shared layout sections. | 2026-03-12 | 46790b6 |
| [src/ui/style.rs](../../../crates/czar-tui/src/ui/style.rs) | Shared styling/formatting helpers for status colors, fuzzy-match highlights, timestamp truncation, and log line normalization. | 2026-03-08 | cdb6126 |
