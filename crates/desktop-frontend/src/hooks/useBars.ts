import { useQuery } from '@tanstack/react-query';
import { fetchBars } from '@/api/endpoints/bars';

export function useBars(symbol: string, timeframe = '1d', days = 365) {
  return useQuery({
    queryKey: ['bars', symbol, timeframe, days],
    queryFn: () => fetchBars(symbol, timeframe, days),
    enabled: !!symbol,
  });
}
