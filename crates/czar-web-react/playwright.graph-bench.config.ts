import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const apiAddr = '127.0.0.1:4013';
const webPort = 4179;

export default defineConfig({
  testDir: './e2e',
  testMatch: ['**/graph-bench.spec.ts'],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 300_000,
  reporter: 'list',
  use: {
    baseURL: `http://127.0.0.1:${webPort}`,
  },
  webServer: [
    {
      command: `just smoke-stack-api-server ${apiAddr} .czar/smoke/graph-bench examples/workflows`,
      cwd: repoRoot,
      url: `http://${apiAddr}/api/workflows?limit=5&offset=0`,
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
    {
      command: `just perf-web-frontend-server http://${apiAddr} ${webPort}`,
      cwd: repoRoot,
      url: `http://127.0.0.1:${webPort}`,
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
  ],
});
