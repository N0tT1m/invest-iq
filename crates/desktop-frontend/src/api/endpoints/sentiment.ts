import client from '@/api/client';
import type { ApiResponse, SentimentVelocity, SocialSentiment } from '@/api/types';

export async function fetchSentimentVelocity(symbol: string): Promise<SentimentVelocity> {
  const { data } = await client.get<ApiResponse<SentimentVelocity>>(`/api/sentiment/${symbol}/velocity`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch sentiment velocity');
  return data.data;
}

export async function fetchSocialSentiment(symbol: string): Promise<SocialSentiment> {
  const { data } = await client.get<ApiResponse<SocialSentiment>>(`/api/sentiment/${symbol}/social`);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch social sentiment');
  return data.data;
}
