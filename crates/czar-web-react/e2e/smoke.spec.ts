import { expect, test } from '@playwright/test';

test('triggering a workflow from the web UI finishes without client errors', async ({ page }) => {
  const clientIssues: string[] = [];
  let optionalFontFailures = 0;
  const isOptionalFontRequest = (url: string) =>
    url.startsWith('https://fonts.googleapis.com/') || url.startsWith('https://fonts.gstatic.com/');

  page.on('console', (message) => {
    if (message.text().includes('Failed to load resource') && optionalFontFailures > 0) {
      return;
    }
    if (message.type() === 'warning' || message.type() === 'error' || message.type() === 'assert') {
      clientIssues.push(`console.${message.type()}: ${message.text()}`);
    }
  });
  page.on('pageerror', (error) => {
    clientIssues.push(`pageerror: ${error.message}`);
  });
  page.on('requestfailed', (request) => {
    if (isOptionalFontRequest(request.url())) {
      optionalFontFailures += 1;
      return;
    }
    clientIssues.push(
      `requestfailed: ${request.method()} ${request.url()} -> ${request.failure()?.errorText ?? 'unknown'}`
    );
  });
  page.on('response', (response) => {
    if (isOptionalFontRequest(response.url())) {
      return;
    }
    if (response.status() >= 400) {
      clientIssues.push(`http ${response.status()}: ${response.request().method()} ${response.url()}`);
    }
  });

  await page.goto('/');
  await expect(page.getByTestId('workflow-item-simple_args')).toBeVisible();

  await page.getByTestId('workflow-item-simple_args').click();
  await page.getByTestId('run-workflow-button').click();

  const overview = page.getByTestId('run-overview');
  await expect(overview).toBeVisible();
  await expect
    .poll(async () => overview.getAttribute('data-run-status'), {
      message: 'run should finish successfully in the detail panel',
    })
    .toBe('success');

  await page.getByTestId('nav-runs').click();
  await expect(page.locator('[data-testid^="run-item-"][data-run-status="success"]').first()).toBeVisible();

  await page.waitForTimeout(500);
  expect(clientIssues).toEqual([]);
});
