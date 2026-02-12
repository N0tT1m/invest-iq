import client from '@/api/client';
import type { ApiResponse, PendingTrade, AgentAnalyticsSummary } from '@/api/types';

export async function fetchPendingTrades(): Promise<PendingTrade[]> {
  const { data } = await client.get<ApiResponse<PendingTrade[]>>('/api/agent/trades');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch agent trades');
  return data.data;
}

export async function reviewTrade(id: string, action: 'approve' | 'reject'): Promise<void> {
  const { data } = await client.post<ApiResponse<null>>(`/api/agent/trades/${id}/review`, { action });
  if (!data.success) throw new Error(data.error ?? 'Review failed');
}

export async function fetchAnalyticsSummary(): Promise<AgentAnalyticsSummary> {
  const { data } = await client.get<ApiResponse<AgentAnalyticsSummary>>('/api/agent/analytics/summary');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch analytics');
  return data.data;
}

export async function fetchSignalDistribution(): Promise<Record<string, number>> {
  const { data } = await client.get<ApiResponse<Record<string, number>>>('/api/agent/analytics/signal-distribution');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch distribution');
  return data.data;
}

export async function fetchRegimeDistribution(): Promise<Record<string, number>> {
  const { data } = await client.get<ApiResponse<Record<string, number>>>('/api/agent/analytics/regime-distribution');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch regimes');
  return data.data;
}

export async function fetchWinRateByRegime(): Promise<Record<string, number>> {
  const { data } = await client.get<ApiResponse<Record<string, number>>>('/api/agent/analytics/win-rate-by-regime');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function fetchWinRateByConviction(): Promise<Record<string, number>> {
  const { data } = await client.get<ApiResponse<Record<string, number>>>('/api/agent/analytics/win-rate-by-conviction');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}
