import client from '@/api/client';
import type {
  ApiResponse, EarningsData, DividendData, OptionsData,
  ShortInterestData, InsiderData, CorrelationData,
} from '@/api/types';

export async function fetchEarnings(symbol: string): Promise<EarningsData> {
  const { data } = await client.get<ApiResponse<EarningsData>>(`/api/earnings/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch earnings');
  return data.data;
}

export async function fetchDividends(symbol: string): Promise<DividendData> {
  const { data } = await client.get<ApiResponse<DividendData>>(`/api/dividends/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch dividends');
  return data.data;
}

export async function fetchOptions(symbol: string): Promise<OptionsData> {
  const { data } = await client.get<ApiResponse<OptionsData>>(`/api/options/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch options');
  return data.data;
}

export async function fetchShortInterest(symbol: string): Promise<ShortInterestData> {
  const { data } = await client.get<ApiResponse<ShortInterestData>>(`/api/short-interest/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch short interest');
  return data.data;
}

export async function fetchInsiders(symbol: string): Promise<InsiderData> {
  const { data } = await client.get<ApiResponse<InsiderData>>(`/api/insiders/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch insider data');
  return data.data;
}

export async function fetchCorrelations(symbol: string): Promise<CorrelationData> {
  const { data } = await client.get<ApiResponse<CorrelationData>>(`/api/correlation/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch correlations');
  return data.data;
}
