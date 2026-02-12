import { useQuery, useMutation } from '@tanstack/react-query';
import { fetchBacktest, runBacktest } from '@/api/endpoints/backtest';

export function useBacktest(symbol: string, days = 365) {
  return useQuery({
    queryKey: ['backtest', symbol, days],
    queryFn: () => fetchBacktest(symbol, days),
    enabled: !!symbol,
  });
}

export function useRunBacktest() {
  return useMutation({
    mutationFn: runBacktest,
  });
}
