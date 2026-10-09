import { test, chromium } from '@playwright/test';
import lighthouse from 'lighthouse';
import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const DEBUG_PORT = 9222;
const BASE_URL = 'http://127.0.0.1:4178';
const REPORT_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../.czar/reports/frontend-perf',
);

test('lighthouse performance audit', async () => {
  const browser = await chromium.launch({
    headless: true,
    args: [`--remote-debugging-port=${DEBUG_PORT}`],
  });

  try {
    const result = await lighthouse(BASE_URL, {
      port: DEBUG_PORT,
      output: 'json',
      logLevel: 'silent',
      formFactor: 'desktop',
      screenEmulation: {
        mobile: false,
        width: 1350,
        height: 940,
        deviceScaleFactor: 1,
        disabled: false,
      },
      throttlingMethod: 'simulate',
      throttling: {
        rttMs: 0,
        throughputKbps: 0,
        cpuSlowdownMultiplier: 4,
        requestLatencyMs: 0,
        downloadThroughputKbps: 0,
        uploadThroughputKbps: 0,
      },
    });

    if (!result) {
      throw new Error('Lighthouse returned no result');
    }

    const lhr = result.lhr;
    const audits = lhr.audits;

    const categories = Object.entries(lhr.categories)
      .map(([, cat]) => `${cat.title}: ${Math.round((cat.score ?? 0) * 100)}/100`)
      .join('  |  ');

    console.log(`\n--- Lighthouse Scores: ${categories} ---`);
    console.log(
      `FCP:         ${audits['first-contentful-paint']?.displayValue ?? 'n/a'}  (${audits['first-contentful-paint']?.score ?? 'n/a'})`,
    );
    console.log(
      `LCP:         ${audits['largest-contentful-paint']?.displayValue ?? 'n/a'}  (${audits['largest-contentful-paint']?.score ?? 'n/a'})`,
    );
    console.log(
      `TBT:         ${audits['total-blocking-time']?.displayValue ?? 'n/a'}  (${audits['total-blocking-time']?.score ?? 'n/a'})`,
    );
    console.log(
      `CLS:         ${audits['cumulative-layout-shift']?.displayValue ?? 'n/a'}  (${audits['cumulative-layout-shift']?.score ?? 'n/a'})`,
    );
    console.log(
      `TTI:         ${audits['interactive']?.displayValue ?? 'n/a'}  (${audits['interactive']?.score ?? 'n/a'})`,
    );
    console.log(
      `Speed Index: ${audits['speed-index']?.displayValue ?? 'n/a'}  (${audits['speed-index']?.score ?? 'n/a'})`,
    );

    mkdirSync(REPORT_DIR, { recursive: true });

    const summary = {
      timestamp: new Date().toISOString(),
      url: BASE_URL,
      scores: Object.fromEntries(
        Object.entries(lhr.categories).map(([id, cat]) => [id, Math.round((cat.score ?? 0) * 100)]),
      ),
      metrics: {
        fcp_ms: Math.round(audits['first-contentful-paint']?.numericValue ?? 0),
        lcp_ms: Math.round(audits['largest-contentful-paint']?.numericValue ?? 0),
        tbt_ms: Math.round(audits['total-blocking-time']?.numericValue ?? 0),
        cls: Number((audits['cumulative-layout-shift']?.numericValue ?? 0).toFixed(3)),
        tti_ms: Math.round(audits['interactive']?.numericValue ?? 0),
        speed_index_ms: Math.round(audits['speed-index']?.numericValue ?? 0),
      },
    };

    writeFileSync(path.join(REPORT_DIR, 'latest.json'), JSON.stringify(summary, null, 2));

    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    writeFileSync(path.join(REPORT_DIR, `lighthouse-${timestamp}.json`), JSON.stringify(lhr, null, 2));

    console.log(`\nFull LHR saved to: .czar/reports/frontend-perf/lighthouse-${timestamp}.json`);
    console.log(`Summary saved to:  .czar/reports/frontend-perf/latest.json\n`);
  } finally {
    await browser.close();
  }
});
