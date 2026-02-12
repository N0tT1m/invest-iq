import client from '@/api/client';
import type { ApiResponse, PortfolioSummary } from '@/api/types';

export async function fetchPortfolioSummary(): Promise<PortfolioSummary> {
  const { data } = await client.get<ApiResponse<PortfolioSummary>>('/api/portfolio');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch portfolio');
  return data.data;
}

export async function fetchRiskMetrics(): Promise<Record<string, unknown>> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>>>('/api/portfolio/risk-metrics');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch risk metrics');
  return data.data;
}

export async function fetchBenchmark(): Promise<Record<string, unknown>> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>>>('/api/portfolio/benchmark');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch benchmark');
  return data.data;
}
