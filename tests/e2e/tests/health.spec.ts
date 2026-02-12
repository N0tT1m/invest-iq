import { test, expect } from '@playwright/test';

const API_URL = process.env.API_URL || 'http://localhost:3000';

test.describe('Health & System', () => {
  test('API health endpoint returns status', async ({ request }) => {
    const response = await request.get(`${API_URL}/health`);
    expect(response.ok() || response.status() === 503).toBeTruthy();
    const body = await response.json();
    expect(body.status).toBeDefined();
    expect(body.service).toBe('invest-iq-api');
    expect(body.checks).toBeDefined();
  });

  test('Metrics endpoint returns Prometheus format', async ({ request }) => {
    const response = await request.get(`${API_URL}/metrics`);
    expect(response.ok()).toBeTruthy();
    const text = await response.text();
    expect(text).toContain('investiq_requests_total');
  });

  test('JSON metrics endpoint returns valid JSON', async ({ request }) => {
    const response = await request.get(`${API_URL}/metrics/json`);
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.request_count).toBeDefined();
  });

  test('OpenAPI docs serve Swagger UI', async ({ request }) => {
    const response = await request.get(`${API_URL}/api-docs/openapi.json`);
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.openapi).toBeDefined();
    expect(body.info.title).toBe('InvestIQ API');
  });

  test('Frontend dashboard page loads', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('body')).toBeVisible();
    // Dashboard should have the stock analysis input
    await expect(page.getByPlaceholder(/symbol|ticker/i).first()).toBeVisible({ timeout: 15000 });
  });
});
