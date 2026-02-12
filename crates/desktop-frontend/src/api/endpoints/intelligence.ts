import client from '@/api/client';
import type {
  ApiResponse, MacroIndicators, WatchlistItem, SectorFlow,
  StrategyHealth, TaxLot, TaxYearEndSummary,
} from '@/api/types';

export async function fetchMacroIndicators(): Promise<MacroIndicators> {
  const { data } = await client.get<ApiResponse<MacroIndicators>>('/api/macro/indicators');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch macro data');
  return data.data;
}

export async function fetchMacroSensitivity(symbol: string): Promise<Record<string, unknown>> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>>>(`/api/macro/sensitivity/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function fetchWatchlist(): Promise<WatchlistItem[]> {
  const { data } = await client.get<ApiResponse<WatchlistItem[]>>('/api/watchlist/items');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch watchlist');
  return data.data;
}

export async function scanOpportunities(): Promise<WatchlistItem[]> {
  const { data } = await client.get<ApiResponse<WatchlistItem[]>>('/api/watchlist/scan');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to scan');
  return data.data;
}

export async function fetchSectorFlows(): Promise<SectorFlow[]> {
  const { data } = await client.get<ApiResponse<SectorFlow[]>>('/api/flows/sectors');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch flows');
  return data.data;
}

export async function fetchAllStrategiesHealth(): Promise<StrategyHealth[]> {
  const { data } = await client.get<ApiResponse<StrategyHealth[]>>('/api/strategies/health');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch strategies');
  return data.data;
}

export async function fetchTaxLots(): Promise<TaxLot[]> {
  const { data } = await client.get<ApiResponse<TaxLot[]>>('/api/tax/lots');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch tax lots');
  return data.data;
}

export async function fetchYearEndSummary(): Promise<TaxYearEndSummary> {
  const { data } = await client.get<ApiResponse<TaxYearEndSummary>>('/api/tax/year-end-summary');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch summary');
  return data.data;
}
