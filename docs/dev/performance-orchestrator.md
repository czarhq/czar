# Orchestrator Performance Testing

_Updated at: 2026-05-22_

Tests the scheduler's throughput, latency, and resource usage under configurable load.

## Running

The `just perf-*` recipes live in [crates/czar-cli/justfile](../../crates/czar-cli/justfile); run them from `crates/czar-cli`. Each `just perf-*` command starts Postgres (via Docker Compose), initializes the database, cleans leftover data, and runs the benchmark with no manual setup needed.

### Predefined Configurations

All configs live in [crates/czar-cli/perf-configs/](../../crates/czar-cli/perf-configs/).

```bash
just perf-quick       # 10 runs × 5 tasks, 0.1s task duration
just perf-standard    # 100 runs × 5 tasks, 0.1s task duration
just perf-latency     # 50 runs × 5 tasks, 0.01s task duration (scheduling overhead focus)
just perf-heavy       # 1000 runs × 10 tasks, 0.05s task duration
just perf-memory      # 100 runs × 20 tasks, 2.0s task duration (resource usage focus)
just perf-all         # run all configs, save timestamped results
```

### Recording Results

```bash
just perf-record                           # append to docs/reports/perf/results.md
just perf-record "after scheduler refactor"  # with a note
```

Commit `docs/reports/perf/results.md` to track performance over time.

## Output

Real-time progress (every 500ms):

```
[1.5s] Completed:0/50 | Pending:0 Dispatched:0 Running:50 | Scheduler: 20000 KB, 30.5% CPU | runs:0.1s/4.2% tasks:0.1s/2.3% updates:0.2s/6.5% sleep:3.0s/85.9% total:3.5s (4 loops, last_ts:...)
```

Final summary:

```
=== Performance Test Results ===

Scheduler Time Breakdown:
  Total scheduler loops: 5
  Time processing runs: 0.15s (3.9%)
  Time processing tasks: 0.08s (2.2%)
  Time processing status updates: 0.53s (13.8%)
  Time sleeping: 3.00s (79.1%)
  Total scheduler time: 3.80s

Latency Stats:
  Tasks: 50
  Avg latency: 0.563s
  Min latency: 0.514s
  Max latency: 0.621s

Throughput:
  Run submission: 292.68 runs/sec
  Task completion: 24.37 tasks/sec
  Total duration: 2.05s

Resource Usage:
  Scheduler RSS: 20000 KB
  Scheduler RSS growth: 2608 KB
```

**Latency** is `created_at` → `finished_at` in the DB; scheduling overhead is included, `czar trigger` round-trip is not.

**Scheduler time breakdown** is parsed from `SCHEDULER_METRICS` JSON log lines emitted by the scheduler loop.

## Custom Configurations

Add a YAML file to [crates/czar-cli/perf-configs/](../../crates/czar-cli/perf-configs/):

```yaml
# perf-configs/my-test.yaml
workflows: 200
tasks_per_workflow: 8
duration: 0.25

scheduler:
  poll_interval_secs: 2
  max_tasks_per_batch: 200
  max_concurrent_dispatches: 50
  max_concurrent_status_checks: 100
```

Then run:
```bash
just perf my-test
```

Or add a `just` target in [crates/czar-cli/justfile](../../crates/czar-cli/justfile).

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `postgres://postgres:postgres@localhost:5432/orchestrator` | Postgres connection string |
| `CZAR_PERF_BIN` | `../../target/release/czar` | Path to the czar binary |
| `CZAR_PERF_BOOT_WAIT_SECS` | `2` | Seconds to wait for the scheduler to start |
| `CZAR_PERF_MONITOR_MAX_POLLS` | unlimited | Stop after N polls (useful for CI timeouts) |

## Implementation

**Binary**: [crates/czar-cli/src/bin/perf-test.rs](../../crates/czar-cli/src/bin/perf-test.rs)

1. Generates a workflow and loads it via `czar load`
2. Starts the scheduler via `czar serve --config <generated-yaml>`
3. Triggers N workflows concurrently via Tokio async tasks
4. Polls the DB every 500ms until all tasks complete
5. Parses `SCHEDULER_METRICS` JSON lines from the scheduler log for time breakdown
6. Queries `created_at`/`finished_at` directly via `sqlx` for latency stats
7. Monitors RSS/CPU via the `sysinfo` crate

**Support modules** (`src/bin/perf_test/`):
- `perf_support.rs`: config loading, workflow/script generation, scheduler config writing, output formatting
- `perf_metrics.rs`: parsing `SCHEDULER_METRICS` log lines, aggregating scheduler time breakdown
- `db_stats.rs`: latency and task count queries
