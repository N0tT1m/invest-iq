import client from '@/api/client';
import type { ApiResponse, SymbolSearchResult, ScreenerResult } from '@/api/types';

export async function searchSymbols(query: string): Promise<SymbolSearchResult[]> {
  const { data } = await client.get<ApiResponse<SymbolSearchResult[]>>('/api/symbols/search', {
    params: { q: query },
  });
  if (!data.success || !data.data) throw new Error(data.error ?? 'Search failed');
  return data.data;
}

export async function fetchSymbolDetail(symbol: string): Promise<Record<string, unknown>> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>>>(`/api/symbols/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function scanStocks(filters: Record<string, unknown>): Promise<ScreenerResult[]> {
  const { data } = await client.post<ApiResponse<ScreenerResult[]>>('/api/screener/scan', filters);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Scan failed');
  return data.data;
}

export async function fetchScreenerPresets(): Promise<Record<string, unknown>[]> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>[]>>('/api/screener/presets');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}
