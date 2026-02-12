import { useQuery } from '@tanstack/react-query';
import { fetchAnalysis } from '@/api/endpoints/analysis';

export function useAnalysis(symbol: string, timeframe = '1d', days = 365) {
  return useQuery({
    queryKey: ['analysis', symbol, timeframe, days],
    queryFn: () => fetchAnalysis(symbol, timeframe, days),
    enabled: !!symbol,
  });
}
