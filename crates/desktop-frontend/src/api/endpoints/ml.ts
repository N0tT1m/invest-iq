import client from '@/api/client';
import type {
  ApiResponse, MLTradeSignal, MLSentimentResult,
  MLPriceForecast, MLCalibrationResult, MLStrategyWeights,
} from '@/api/types';

export async function fetchMLTradeSignal(symbol: string): Promise<MLTradeSignal> {
  const { data } = await client.get<ApiResponse<MLTradeSignal>>(`/api/ml/trade-signal/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch ML signal');
  return data.data;
}

export async function fetchMLSentiment(symbol: string): Promise<MLSentimentResult> {
  const { data } = await client.get<ApiResponse<MLSentimentResult>>(`/api/ml/sentiment/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function fetchMLPriceForecast(symbol: string): Promise<MLPriceForecast> {
  const { data } = await client.get<ApiResponse<MLPriceForecast>>(`/api/ml/price-forecast/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function fetchMLCalibration(symbol: string): Promise<MLCalibrationResult> {
  const { data } = await client.get<ApiResponse<MLCalibrationResult>>(`/api/ml/calibration/${symbol}`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}

export async function fetchMLStrategyWeights(): Promise<MLStrategyWeights> {
  const { data } = await client.get<ApiResponse<MLStrategyWeights>>('/api/ml/strategy-weights');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed');
  return data.data;
}
