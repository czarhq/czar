/**
 * CDP-based graph rendering benchmark for large workflows.
 *
 * Measures time-to-first-node and time-to-all-nodes for deep_graph (1000 tasks,
 * 100 layers × 10 nodes) and wide_graph (1000 tasks, 10 layers × 100 nodes).
 *
 * Each workflow is measured twice:
 *   - collapsed: default auto-collapsed state (group nodes only) — the common case
 *   - expanded:  all groups expanded, full 1000 individual task nodes visible
 *
 * Run via: just graph-bench
 * Results saved to: .czar/reports/graph-bench/latest.json
 */

import { test, expect, type Page } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const REPORT_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../.czar/reports/graph-bench',
);

const NODE_SELECTOR = '.react-flow__node';
const ALL_NODES_TIMEOUT = 60_000;

interface GraphBenchResult {
  workflow: string;
  state: 'collapsed' | 'expanded';
  ttfn_ms: number;
  ttan_ms: number;
  node_count: number;
  js_heap_used_mb: number;
  js_heap_total_mb: number;
  layout_count: number;
  style_count: number;
}

async function measureGraphLoad(
  page: Page,
  workflowName: string,
  state: 'collapsed' | 'expanded',
  expectedNodeCount: number,
): Promise<GraphBenchResult> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');

  const t0 = Date.now();

  const groupsParam = state === 'expanded' ? '&groups=expanded' : '';
  await page.goto(`/?workflow=${encodeURIComponent(workflowName)}&tab=graph${groupsParam}`, { waitUntil: 'domcontentloaded' });

  // Time to first node
  await page.waitForSelector(NODE_SELECTOR, { timeout: ALL_NODES_TIMEOUT });
  const ttfn = Date.now() - t0;

  // Time to all nodes
  await page.waitForFunction(
    ({ selector, count }: { selector: string; count: number }) =>
      document.querySelectorAll(selector).length >= count,
    { selector: NODE_SELECTOR, count: expectedNodeCount },
    { timeout: ALL_NODES_TIMEOUT },
  );
  const ttan = Date.now() - t0;

  const actualCount = await page.evaluate(
    (selector: string) => document.querySelectorAll(selector).length,
    NODE_SELECTOR,
  );

  const perfMetrics = await cdp.send('Performance.getMetrics');
  const metric = (name: string): number =>
    perfMetrics.metrics.find((m: { name: string; value: number }) => m.name === name)?.value ?? 0;

  return {
    workflow: workflowName,
    state,
    ttfn_ms: ttfn,
    ttan_ms: ttan,
    node_count: actualCount,
    js_heap_used_mb: Math.round((metric('JSHeapUsedSize') / 1024 / 1024) * 10) / 10,
    js_heap_total_mb: Math.round((metric('JSHeapTotalSize') / 1024 / 1024) * 10) / 10,
    layout_count: metric('LayoutCount'),
    style_count: metric('RecalcStyleCount'),
  };
}

const workflows = [
  // collapsed node count = number of groups (layers)
  { name: 'deep_graph', description: '1 000 tasks, 100 layers × 10 nodes', collapsedCount: 100, expandedCount: 1000 },
  { name: 'wide_graph', description: '1 000 tasks, 10 layers × 100 nodes', collapsedCount: 10, expandedCount: 1000 },
];

const results: GraphBenchResult[] = [];

for (const { name, description, collapsedCount, expandedCount } of workflows) {
  test(`graph render benchmark: ${name} (collapsed)`, async ({ page }) => {
    console.log(`\n--- ${name} [collapsed]: ${description} ---`);
    const result = await measureGraphLoad(page, name, 'collapsed', collapsedCount);
    results.push(result);
    printResult(result);
    expect(result.node_count).toBeGreaterThanOrEqual(collapsedCount);
  });

  test(`graph render benchmark: ${name} (expanded)`, async ({ page }) => {
    console.log(`\n--- ${name} [expanded]: ${description} ---`);
    const result = await measureGraphLoad(page, name, 'expanded', expandedCount);
    results.push(result);
    printResult(result);
    expect(result.node_count).toBeGreaterThanOrEqual(expandedCount);
  });
}

function printResult(r: GraphBenchResult) {
  console.log(`  TTFN (time-to-first-node): ${r.ttfn_ms} ms`);
  console.log(`  TTAN (time-to-all-nodes):  ${r.ttan_ms} ms`);
  console.log(`  Node count:                ${r.node_count}`);
  console.log(`  JS heap used:              ${r.js_heap_used_mb} MB`);
  console.log(`  JS heap total:             ${r.js_heap_total_mb} MB`);
  console.log(`  Layout count:              ${r.layout_count}`);
  console.log(`  Style recalc count:        ${r.style_count}`);
}

test.afterAll(() => {
  if (results.length === 0) return;

  mkdirSync(REPORT_DIR, { recursive: true });

  const summary = { timestamp: new Date().toISOString(), results };
  writeFileSync(path.join(REPORT_DIR, 'latest.json'), JSON.stringify(summary, null, 2));

  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  writeFileSync(
    path.join(REPORT_DIR, `graph-bench-${timestamp}.json`),
    JSON.stringify(summary, null, 2),
  );

  console.log(`\nSummary saved to: .czar/reports/graph-bench/latest.json`);
});
