import { useQuery } from '@tanstack/react-query';
import {
  fetchMacroIndicators, fetchMacroSensitivity, fetchWatchlist,
  scanOpportunities, fetchSectorFlows, fetchAllStrategiesHealth,
  fetchTaxLots, fetchYearEndSummary,
} from '@/api/endpoints/intelligence';
import { fetchSocialSentiment } from '@/api/endpoints/sentiment';

export function useMacroIndicators() {
  return useQuery({ queryKey: ['macroIndicators'], queryFn: fetchMacroIndicators });
}

export function useMacroSensitivity(symbol: string) {
  return useQuery({ queryKey: ['macroSensitivity', symbol], queryFn: () => fetchMacroSensitivity(symbol), enabled: !!symbol });
}

export function useWatchlist() {
  return useQuery({ queryKey: ['watchlist'], queryFn: fetchWatchlist });
}

export function useOpportunities() {
  return useQuery({ queryKey: ['opportunities'], queryFn: scanOpportunities });
}

export function useSectorFlows() {
  return useQuery({ queryKey: ['sectorFlows'], queryFn: fetchSectorFlows });
}

export function useStrategiesHealth() {
  return useQuery({ queryKey: ['strategiesHealth'], queryFn: fetchAllStrategiesHealth });
}

export function useTaxLots() {
  return useQuery({ queryKey: ['taxLots'], queryFn: fetchTaxLots });
}

export function useYearEndSummary() {
  return useQuery({ queryKey: ['yearEndSummary'], queryFn: fetchYearEndSummary });
}

export function useSocialSentimentIntel(symbol: string) {
  return useQuery({ queryKey: ['socialSentiment', symbol], queryFn: () => fetchSocialSentiment(symbol), enabled: !!symbol });
}
