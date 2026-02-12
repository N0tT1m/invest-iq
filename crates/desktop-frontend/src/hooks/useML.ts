import { useQuery } from '@tanstack/react-query';
import {
  fetchMLTradeSignal, fetchMLSentiment, fetchMLPriceForecast,
  fetchMLCalibration, fetchMLStrategyWeights,
} from '@/api/endpoints/ml';

export function useMLTradeSignal(symbol: string) {
  return useQuery({ queryKey: ['mlSignal', symbol], queryFn: () => fetchMLTradeSignal(symbol), enabled: !!symbol });
}

export function useMLSentiment(symbol: string) {
  return useQuery({ queryKey: ['mlSentiment', symbol], queryFn: () => fetchMLSentiment(symbol), enabled: !!symbol });
}

export function useMLPriceForecast(symbol: string) {
  return useQuery({ queryKey: ['mlForecast', symbol], queryFn: () => fetchMLPriceForecast(symbol), enabled: !!symbol });
}

export function useMLCalibration(symbol: string) {
  return useQuery({ queryKey: ['mlCalibration', symbol], queryFn: () => fetchMLCalibration(symbol), enabled: !!symbol });
}

export function useMLStrategyWeights() {
  return useQuery({ queryKey: ['mlWeights'], queryFn: fetchMLStrategyWeights });
}
