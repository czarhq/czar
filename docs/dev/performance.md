# Performance Testing

_Updated at: 2026-05-22_

Czar has two distinct performance testing areas, each with its own tooling and metrics.

## Areas

| Area | What it measures | Deep-dive |
|---|---|---|
| **Orchestrator** | Scheduler throughput, task latency, resource usage | [performance-orchestrator.md](performance-orchestrator.md) |
| **Frontend** | Page load, graph render time, JS heap | [performance-frontend.md](performance-frontend.md) |

## Quick Reference

```bash
# Orchestrator: scheduler throughput/latency (requires Docker for Postgres; run from crates/czar-cli)
just perf-quick          # smoke: 10 runs × 5 tasks
just perf-standard       # typical: 100 runs × 5 tasks
just perf-heavy          # stress: 1000 runs × 10 tasks
just perf-all            # run all configs

# Frontend: Lighthouse page load audit (requires smoke stack)
just perf-frontend

# Frontend: graph rendering benchmark, 1000-node DAGs
just graph-bench
```

## Reports

All results are written to `.czar/reports/`:

| Path | Content |
|---|---|
| `.czar/reports/frontend-perf/latest.json` | Latest Lighthouse audit |
| `.czar/reports/frontend-perf/lighthouse-<timestamp>.json` | Archived Lighthouse runs |
| `.czar/reports/graph-bench/latest.json` | Latest graph render benchmark |
| `.czar/reports/graph-bench/graph-bench-<timestamp>.json` | Archived graph bench runs |
| `docs/reports/perf/results.md` | Orchestrator perf record (committed, tracked over time) |

## Metric Glossary

### Frontend

| Metric | Full name | What it means |
|---|---|---|
| **TTFN** | Time to first node | Milliseconds from page navigation start until the first graph node appears in the DOM |
| **TTAN** | Time to all nodes | Milliseconds from page navigation start until all expected nodes are in the DOM |
| **FCP** | First Contentful Paint | Time until the browser renders the first text or image |
| **LCP** | Largest Contentful Paint | Time until the largest visible content element is rendered |
| **TBT** | Total Blocking Time | Total time the main thread was blocked (>50 ms tasks), directly impacts interactivity |
| **CLS** | Cumulative Layout Shift | Visual stability score (0 = no shifting, lower is better) |
| **TTI** | Time to Interactive | Time until the page is reliably interactive |
| **SI** | Speed Index | How quickly content is visually populated (lower is better) |

### Orchestrator

| Metric | What it means |
|---|---|
| **Throughput** | Completed tasks per second at steady state |
| **Latency** | `created_at` → `finished_at` in the DB; includes scheduling overhead, excludes trigger round-trip |
| **RSS** | Resident set size: scheduler process memory in KB |
| **Scheduler time breakdown** | Per-loop time split across runs/tasks/status-updates/sleep, parsed from `SCHEDULER_METRICS` log lines |
