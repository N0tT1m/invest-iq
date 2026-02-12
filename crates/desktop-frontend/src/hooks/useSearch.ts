import { useQuery, useMutation } from '@tanstack/react-query';
import { searchSymbols, scanStocks, fetchScreenerPresets } from '@/api/endpoints/search';

export function useSymbolSearch(query: string) {
  return useQuery({
    queryKey: ['symbolSearch', query],
    queryFn: () => searchSymbols(query),
    enabled: query.length >= 1,
  });
}

export function useScreenerPresets() {
  return useQuery({ queryKey: ['screenerPresets'], queryFn: fetchScreenerPresets });
}

export function useScanStocks() {
  return useMutation({ mutationFn: scanStocks });
}
