import client from '@/api/client';
import type { ApiResponse, CalibrationResult } from '@/api/types';

export async function fetchCalibrationStats(): Promise<Record<string, unknown>> {
  const { data } = await client.get<ApiResponse<Record<string, unknown>>>('/api/calibration/stats');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch calibration stats');
  return data.data;
}

export async function calibratePrediction(rawConfidence: number, symbol?: string): Promise<CalibrationResult> {
  const { data } = await client.post<ApiResponse<CalibrationResult>>('/api/calibration/calibrate', {
    raw_confidence: rawConfidence,
    symbol,
  });
  if (!data.success || !data.data) throw new Error(data.error ?? 'Calibration failed');
  return data.data;
}
