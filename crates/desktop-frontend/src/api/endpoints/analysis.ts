import client from '@/api/client';
import type { ApiResponse, AnalysisResult } from '@/api/types';

export async function fetchAnalysis(symbol: string, timeframe = '1d', days = 365): Promise<AnalysisResult> {
  const { data } = await client.get<ApiResponse<AnalysisResult>>(
    `/api/analyze/${symbol}`,
    { params: { timeframe, days } },
  );
  if (!data.success || !data.data) throw new Error(data.error ?? 'Analysis failed');
  return data.data;
}
