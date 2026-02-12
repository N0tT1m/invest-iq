import { useQuery } from '@tanstack/react-query';
import {
  fetchEarnings, fetchDividends, fetchOptions,
  fetchShortInterest, fetchInsiders, fetchCorrelations,
} from '@/api/endpoints/research';

export function useEarnings(symbol: string) {
  return useQuery({ queryKey: ['earnings', symbol], queryFn: () => fetchEarnings(symbol), enabled: !!symbol });
}

export function useDividends(symbol: string) {
  return useQuery({ queryKey: ['dividends', symbol], queryFn: () => fetchDividends(symbol), enabled: !!symbol });
}

export function useOptions(symbol: string) {
  return useQuery({ queryKey: ['options', symbol], queryFn: () => fetchOptions(symbol), enabled: !!symbol });
}

export function useShortInterest(symbol: string) {
  return useQuery({ queryKey: ['shortInterest', symbol], queryFn: () => fetchShortInterest(symbol), enabled: !!symbol });
}

export function useInsiders(symbol: string) {
  return useQuery({ queryKey: ['insiders', symbol], queryFn: () => fetchInsiders(symbol), enabled: !!symbol });
}

export function useCorrelations(symbol: string) {
  return useQuery({ queryKey: ['correlations', symbol], queryFn: () => fetchCorrelations(symbol), enabled: !!symbol });
}
