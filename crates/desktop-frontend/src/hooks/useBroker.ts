import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { fetchAccount, fetchBrokerPositions, fetchOrders, executeTrade, cancelOrder, closePosition } from '@/api/endpoints/broker';
import type { TradeRequest } from '@/api/types';

export function useAccount() {
  return useQuery({ queryKey: ['brokerAccount'], queryFn: fetchAccount });
}

export function useBrokerPositions() {
  return useQuery({ queryKey: ['brokerPositions'], queryFn: fetchBrokerPositions });
}

export function useOrders() {
  return useQuery({ queryKey: ['brokerOrders'], queryFn: fetchOrders });
}

export function useExecuteTrade() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (trade: TradeRequest) => executeTrade(trade),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['brokerAccount'] });
      qc.invalidateQueries({ queryKey: ['brokerPositions'] });
      qc.invalidateQueries({ queryKey: ['brokerOrders'] });
    },
  });
}

export function useCancelOrder() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (orderId: string) => cancelOrder(orderId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['brokerOrders'] }),
  });
}

export function useClosePosition() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (symbol: string) => closePosition(symbol),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['brokerPositions'] });
      qc.invalidateQueries({ queryKey: ['brokerAccount'] });
    },
  });
}
