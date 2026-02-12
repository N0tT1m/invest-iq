import { test, expect } from '@playwright/test';

const API_URL = process.env.API_URL || 'http://localhost:3000';
const API_KEY = process.env.API_KEY || '';

test.describe('Backtest Flow', () => {
  test('API backtest endpoint returns results', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/backtest/AAPL?days=180`, {
      headers,
      timeout: 60000,
    });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    expect(body.data.total_trades).toBeGreaterThanOrEqual(0);
    expect(body.data.equity_curve).toBeDefined();
  });

  test('Frontend backtest tab exists', async ({ page }) => {
    await page.goto('/');
    const backtestTab = page.locator('text=/Backtest/i').first();
    await expect(backtestTab).toBeVisible({ timeout: 15000 });
  });
});
