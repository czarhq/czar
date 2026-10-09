# Postgres Fault Injection Tests

_Updated at: 2026-05-22_

This document explains the current Toxiproxy-backed Postgres fault-injection coverage in Czar.

## Scope

These tests are currently **scheduler integration tests**, not full-stack end-to-end tests.

The main test file is:

- [crates/czar-tests/tests/core/scheduler_processing_behavior_test/postgres_faults.rs](../../crates/czar-tests/tests/core/scheduler_processing_behavior_test/postgres_faults.rs)

The shared disposable Postgres harness lives in:

- [crates/czar-store-postgres/tests/support/postgres_docker.rs](../../crates/czar-store-postgres/tests/support/postgres_docker.rs)

What is under test:

- the scheduler loop in `czar-core`
- a real `PostgresDatabase` backend from `czar-store`
- fault injection on the scheduler's database connection via Toxiproxy

What is not under test:

- browser or UI behavior
- the full `just up` stack
- API responses during DB degradation
- general production topology

## Topology

The tests use two Postgres connections on purpose:

- `proxied_db`: used by the scheduler under test
- `direct_db`: used by test setup, assertions, and cleanup

The effective topology is:

```text
test assertions/setup ----> Postgres
scheduler under test ----> Toxiproxy ----> Postgres
```

This split matters because the test still needs to inspect run state and clean up the database even while the scheduler's DB path is intentionally degraded.

## Faults Covered Today

The current coverage is intentionally narrow and deterministic.

### 1. Full Proxy Disconnect

Test:

- `test_scheduler_recovers_after_toxiproxy_disconnect`

Mechanism:

- Toxiproxy is left in the connection path, but the proxy is disabled with `enabled = false`

What this simulates:

- the scheduler temporarily losing database connectivity entirely

What the test asserts:

- the scheduler task does not crash while the DB path is unavailable
- after the proxy is re-enabled, the scheduler resumes processing
- a pending DAG run eventually reaches `success`

### 2. Latency Spike

Test:

- `test_scheduler_recovers_after_toxiproxy_latency_spike`

Mechanism:

- a downstream latency toxic is added with:
  - latency: `1500 ms`
  - jitter: `0`

What this simulates:

- a severe but deterministic slowdown in database responses

What the test asserts:

- the scheduler task does not exit during the slowdown
- after the toxic is removed, the scheduler resumes normal processing
- a pending DAG run eventually reaches `success`

## What These Tests Prove

- Czar can exercise scheduler resilience against real Postgres connectivity degradation instead of mocking DB failures.
- The scheduler currently survives the two injected fault modes above without the task itself crashing.
- Recovery is validated by observing a real persisted run transition to `success` after the fault is removed.

## What These Tests Do Not Yet Prove

- user-visible behavior in the web UI
- API-level degradation or recovery behavior
- retry/backoff quality under long outages
- packet loss, bandwidth throttling, timeouts, connection resets, or half-open socket behavior
- behavior under repeated or overlapping fault patterns

## Why Toxiproxy Is Used Here

Toxiproxy is being used as a **test/development fault-injection layer**, not as an application runtime dependency.

In this repo, the current use is appropriate for:

- deterministic scheduler integration tests
- future opt-in local development drills
- future end-to-end tests that want to verify full-stack UX during DB degradation

It is not currently wired into normal runtime or production operation.

## Practical Reading Guide

If you are reading the implementation for the first time:

1. Start with [crates/czar-tests/tests/core/scheduler_processing_behavior_test/postgres_faults.rs](../../crates/czar-tests/tests/core/scheduler_processing_behavior_test/postgres_faults.rs) to see the exact injected faults and assertions.
2. Then read [crates/czar-store-postgres/tests/support/postgres_docker.rs](../../crates/czar-store-postgres/tests/support/postgres_docker.rs) to understand how the disposable Postgres server is provisioned.
3. Treat these tests as scheduler integration coverage, not as evidence of full-stack UX behavior.
