import client from '@/api/client';
import type { ApiResponse, BacktestResult } from '@/api/types';

export async function fetchBacktest(symbol: string, days = 365): Promise<BacktestResult> {
  const { data } = await client.get<ApiResponse<BacktestResult>>(
    `/api/backtest/${symbol}`,
    { params: { days } },
  );
  if (!data.success || !data.data) throw new Error(data.error ?? 'Backtest failed');
  return data.data;
}

export async function runBacktest(params: {
  symbol: string;
  strategy?: string;
  days?: number;
  initial_capital?: number;
}): Promise<BacktestResult> {
  const { data } = await client.post<ApiResponse<BacktestResult>>('/api/backtest/run', params);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Backtest run failed');
  return data.data;
}

export async function fetchAllBacktests(): Promise<BacktestResult[]> {
  const { data } = await client.get<ApiResponse<BacktestResult[]>>('/api/backtest/results');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch backtests');
  return data.data;
}
