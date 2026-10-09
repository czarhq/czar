import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const apiAddr = '127.0.0.1:4010';
const webPort = 4176;

export default defineConfig({
  testDir: './e2e',
  testMatch: ['**/smoke.spec.ts'],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 90_000,
  expect: {
    timeout: 45_000,
  },
  reporter: 'list',
  use: {
    baseURL: `http://127.0.0.1:${webPort}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  webServer: [
    {
      command: `just smoke-stack-api-server ${apiAddr} .czar/smoke/web examples/workflows/simple_args`,
      cwd: repoRoot,
      url: `http://${apiAddr}/api/workflows?limit=5&offset=0`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
    {
      command: `just smoke-web-frontend-server http://${apiAddr} ${webPort}`,
      cwd: repoRoot,
      url: `http://127.0.0.1:${webPort}`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
