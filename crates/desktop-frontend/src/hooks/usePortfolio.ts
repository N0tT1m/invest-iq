import { useQuery } from '@tanstack/react-query';
import { fetchPortfolioSummary, fetchRiskMetrics, fetchBenchmark } from '@/api/endpoints/portfolio';

export function usePortfolioSummary() {
  return useQuery({ queryKey: ['portfolio'], queryFn: fetchPortfolioSummary });
}

export function usePortfolioRiskMetrics() {
  return useQuery({ queryKey: ['portfolioRisk'], queryFn: fetchRiskMetrics });
}

export function usePortfolioBenchmark() {
  return useQuery({ queryKey: ['portfolioBenchmark'], queryFn: fetchBenchmark });
}
