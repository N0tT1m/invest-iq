import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  fetchPendingTrades,
  reviewTrade,
  fetchAnalyticsSummary,
  fetchSignalDistribution,
  fetchRegimeDistribution,
  fetchWinRateByRegime,
  fetchWinRateByConviction,
} from '@/api/endpoints/agent';

export function usePendingTrades() {
  return useQuery({ queryKey: ['agentTrades'], queryFn: fetchPendingTrades });
}

export function useReviewTrade() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, action }: { id: string; action: 'approve' | 'reject' }) => reviewTrade(id, action),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['agentTrades'] }),
  });
}

export function useAgentAnalyticsSummary() {
  return useQuery({ queryKey: ['agentAnalytics'], queryFn: fetchAnalyticsSummary });
}

export function useSignalDistribution() {
  return useQuery({ queryKey: ['signalDistribution'], queryFn: fetchSignalDistribution });
}

export function useRegimeDistribution() {
  return useQuery({ queryKey: ['regimeDistribution'], queryFn: fetchRegimeDistribution });
}

export function useWinRateByRegime() {
  return useQuery({ queryKey: ['winRateByRegime'], queryFn: fetchWinRateByRegime });
}

export function useWinRateByConviction() {
  return useQuery({ queryKey: ['winRateByConviction'], queryFn: fetchWinRateByConviction });
}
