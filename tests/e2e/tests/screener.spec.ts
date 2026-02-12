import { test, expect } from '@playwright/test';

const API_URL = process.env.API_URL || 'http://localhost:3000';
const API_KEY = process.env.API_KEY || '';

test.describe('Symbol Search & Screener', () => {
  test('Symbol search endpoint returns results', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/symbols/search?q=apple&limit=5`, { headers });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
    expect(body.data.length).toBeGreaterThan(0);
  });

  test('Symbol detail endpoint returns ticker info', async ({ request }) => {
    const headers: Record<string, string> = {};
    if (API_KEY) headers['X-API-Key'] = API_KEY;

    const response = await request.get(`${API_URL}/api/symbols/AAPL`, { headers });
    if (response.status() === 401 || response.status() === 403) {
      test.skip(true, 'API key not configured');
      return;
    }
    expect(response.ok()).toBeTruthy();
    const body = await response.json();
    expect(body.success).toBe(true);
  });
});
