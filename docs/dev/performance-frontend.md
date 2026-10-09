# Frontend Performance Testing

_Updated at: 2026-05-22_

Two complementary benchmarks cover different aspects of frontend performance.

## Benchmarks

### 1. Lighthouse Audit (`just perf-frontend`)

Measures page load quality using Google Lighthouse via Playwright. Runs against a production build served against the smoke stack.

**What it covers**: FCP, LCP, TBT, CLS, TTI, Speed Index, and overall Lighthouse scores (Performance, Accessibility, Best Practices, SEO).

**When to use**: After changes to the build, bundle size, asset loading, initial render path, or layout stability.

**Setup**:
```bash
just perf-frontend-install   # first time only (installs Playwright + Chromium)
just perf-frontend           # spins up smoke stack + production frontend automatically
```

Results saved to `.czar/reports/frontend-perf/latest.json` and a timestamped archive.

**Baseline** (2026-03-31, production build):
```
Performance: 100  Accessibility: 98  Best Practices: 100  SEO: 82
FCP: 114ms  LCP: 333ms  TBT: 0ms  CLS: 0.008  TTI: 333ms  SI: 211ms
```

---

### 2. Graph Render Benchmark (`just graph-bench`)

Measures how long it takes to render 1000-node DAGs using Chrome DevTools Protocol (CDP). Tests two graph shapes that stress different aspects of the dagre layout algorithm.

**What it covers**: TTFN (time to first node), TTAN (time to all nodes), JS heap usage, layout count, style recalc count.

**When to use**: After changes to the graph rendering pipeline: layout worker, node decoration, ReactFlow updates, or group collapsing logic.

```bash
just graph-bench   # spins up smoke stack + production frontend automatically
```

Results saved to `.czar/reports/graph-bench/latest.json` and a timestamped archive.

**Graph shapes**:

| Workflow | Shape | Description |
|---|---|---|
| `deep_graph` | 100 layers × 10 nodes | Sequential/deep; stresses dagre's longest-path ranking |
| `wide_graph` | 10 layers × 100 nodes | Parallel/wide; stresses horizontal layout and edge routing |

Both are generated from the existing workflow examples:

- [`examples/workflows/deep_graph/deep_graph.yaml`](../../examples/workflows/deep_graph/deep_graph.yaml)
- [`examples/workflows/wide_graph/wide_graph.yaml`](../../examples/workflows/wide_graph/wide_graph.yaml)

**Latest results** (2026-04-05, production build, post-Worker + slim-poll optimizations):

| | deep_graph | wide_graph |
|---|---|---|
| TTFN | 904ms | 792ms |
| TTAN | 1222ms | 1135ms |
| Node count | 1100 | 1010 |
| Heap used | 42 MB | 49 MB |
| Layout count | 116 | 27 |
| Style recalcs | 134 | 43 |

Node count exceeds 1000 because task groups add group-frame nodes on top of individual task nodes.

---

## Metrics Reference

| Metric | Full name | What it measures |
|---|---|---|
| **TTFN** | Time to first node | ms from navigation start until the first `.react-flow__node` appears in the DOM |
| **TTAN** | Time to all nodes | ms from navigation start until all expected nodes are in the DOM |
| **FCP** | First Contentful Paint | Time until the browser renders the first text or image element |
| **LCP** | Largest Contentful Paint | Time until the largest visible content element is rendered |
| **TBT** | Total Blocking Time | Cumulative time the main thread was blocked (tasks >50ms); directly impacts interactivity |
| **CLS** | Cumulative Layout Shift | Visual stability: 0 means no layout shift, lower is better |
| **TTI** | Time to Interactive | Time until the page is reliably interactive (long tasks have settled) |
| **SI** | Speed Index | How quickly visible content is populated (lower is better) |

## Architecture Notes

**Graph layout is off the main thread.** The dagre layout runs in a Web Worker (`graphLayout.worker.ts`). The main thread posts `GraphNode[]` to the worker and receives positioned `Node[]` + `Edge[]` back. This keeps the UI responsive during layout of large graphs.

**Slim polling omits heavy fields.** Status polls use `?slim=true` to exclude `logs`, `output`, and `error` from task responses. These fields are only fetched on initial load and preserved in memory across polls, avoiding MB of JSON on every 5s interval for runs with many finished tasks.

## Implementation

- Lighthouse spec: [`e2e/perf.spec.ts`](../../crates/czar-web-react/e2e/perf.spec.ts)
- Graph bench spec: [`e2e/graph-bench.spec.ts`](../../crates/czar-web-react/e2e/graph-bench.spec.ts)
- Playwright configs: [`playwright.perf.config.ts`](../../crates/czar-web-react/playwright.perf.config.ts), [`playwright.graph-bench.config.ts`](../../crates/czar-web-react/playwright.graph-bench.config.ts)
- Graph layout worker: [`src/components/graphLayout.worker.ts`](../../crates/czar-web-react/src/components/graphLayout.worker.ts)
- Slim poll handler: [`crates/czar-web/src/handlers/run_detail.rs`](../../crates/czar-web/src/handlers/run_detail.rs)
