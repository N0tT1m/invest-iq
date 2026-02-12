import client from '@/api/client';
import type { ApiResponse, RiskRadar } from '@/api/types';

export async function fetchRiskRadar(symbol: string): Promise<RiskRadar> {
  const { data } = await client.get<ApiResponse<RiskRadar>>(`/api/risk/radar/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch risk radar');
  return data.data;
}

export async function fetchPortfolioRiskRadar(): Promise<RiskRadar> {
  const { data } = await client.get<ApiResponse<RiskRadar>>('/api/risk/radar');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch portfolio risk');
  return data.data;
}
