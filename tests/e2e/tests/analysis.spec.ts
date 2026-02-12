import { test, expect } from '@playwright/test';

const API_URL = process.env.API_URL || 'http://localhost:3000';
const API_KEY = process.env.API_KEY || '';

test.describe('Analysis Flow', () => {
  test('API analyze endpoint returns analysis for valid symbol', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/analyze/AAPL`, { headers });
    // May fail if no API key or Polygon down — skip gracefully
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    expect(body.data.symbol).toBe('AAPL');
    expect(body.data.overall_signal).toBeDefined();
    expect(body.data.overall_confidence).toBeGreaterThan(0);
  });

  test('API bars endpoint returns OHLCV data', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/bars/AAPL?days=30`, { headers });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    expect(body.data.length).toBeGreaterThan(0);
    expect(body.data[0].open).toBeDefined();
    expect(body.data[0].close).toBeDefined();
  });

  test('Frontend triggers analysis and shows results', async ({ page }) => {
    await page.goto('/');

    // Find the symbol input and type a ticker
    const symbolInput = page.getByPlaceholder(/symbol|ticker/i).first();
    await expect(symbolInput).toBeVisible({ timeout: 15000 });
    await symbolInput.fill('AAPL');

    // Click analyze button
    const analyzeBtn = page.getByRole('button', { name: /analyze/i }).first();
    await analyzeBtn.click();

    // Wait for results to load (may take time if API is live)
    // Look for any signal indicator or analysis card
    await expect(
      page.locator('text=/Strong Buy|Buy|Weak Buy|Neutral|Weak Sell|Sell|Strong Sell/i').first()
    ).toBeVisible({ timeout: 45000 });
  });
});
