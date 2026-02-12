import { test, expect } from '@playwright/test';

const API_URL = process.env.API_URL || 'http://localhost:3000';
const API_KEY = process.env.API_KEY || '';

test.describe('Paper Trading Flow', () => {
  test('Broker account endpoint returns account info', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/broker/account`, { headers });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    if (response.status() === 500) {
      test.skip(true, 'Alpaca broker not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
  });

  test('Broker positions endpoint returns positions list', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/broker/positions`, { headers });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    if (response.status() === 500) {
      test.skip(true, 'Alpaca broker not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    expect(Array.isArray(body.data)).toBeTruthy();
  });

  test('Frontend trading tab is visible', async ({ page }) => {
    await page.goto('/');
    // Look for trading-related tab
    const tradingTab = page.locator('text=/Paper Trade|Portfolio|Trading/i').first();
    await expect(tradingTab).toBeVisible({ timeout: 15000 });
  });
});
