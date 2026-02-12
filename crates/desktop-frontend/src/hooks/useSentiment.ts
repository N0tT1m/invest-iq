import { useQuery } from '@tanstack/react-query';
import { fetchSentimentVelocity, fetchSocialSentiment } from '@/api/endpoints/sentiment';

export function useSentimentVelocity(symbol: string) {
  return useQuery({
    queryKey: ['sentimentVelocity', symbol],
    queryFn: () => fetchSentimentVelocity(symbol),
    enabled: !!symbol,
  });
}

export function useSocialSentiment(symbol: string) {
  return useQuery({
    queryKey: ['socialSentiment', symbol],
    queryFn: () => fetchSocialSentiment(symbol),
    enabled: !!symbol,
  });
}
