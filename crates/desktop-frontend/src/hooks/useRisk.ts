import { useQuery } from '@tanstack/react-query';
import { fetchRiskRadar } from '@/api/endpoints/risk';

export function useRiskRadar(symbol: string) {
  return useQuery({
    queryKey: ['riskRadar', symbol],
    queryFn: () => fetchRiskRadar(symbol),
    enabled: !!symbol,
  });
}
