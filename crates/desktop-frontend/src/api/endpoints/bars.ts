import client from '@/api/client';
import type { ApiResponse, BarsResponse } from '@/api/types';

export async function fetchBars(symbol: string, timeframe = '1d', days = 365): Promise<BarsResponse> {
  const { data } = await client.get<ApiResponse<BarsResponse>>(
    `/api/bars/${symbol}`,
    { params: { timeframe, days } },
  );
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch bars');
  return data.data;
}
