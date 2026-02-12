import client from '@/api/client';
import type { ApiResponse, BrokerAccount, BrokerPosition, BrokerOrder, TradeRequest } from '@/api/types';

export async function fetchAccount(): Promise<BrokerAccount> {
  const { data } = await client.get<ApiResponse<BrokerAccount>>('/api/broker/account');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch account');
  return data.data;
}

export async function fetchBrokerPositions(): Promise<BrokerPosition[]> {
  const { data } = await client.get<ApiResponse<BrokerPosition[]>>('/api/broker/positions');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch positions');
  return data.data;
}

export async function fetchOrders(): Promise<BrokerOrder[]> {
  const { data } = await client.get<ApiResponse<BrokerOrder[]>>('/api/broker/orders');
  if (!data.success || !data.data) throw new Error(data.error ?? 'Failed to fetch orders');
  return data.data;
}

export async function executeTrade(trade: TradeRequest): Promise<BrokerOrder> {
  const { data } = await client.post<ApiResponse<BrokerOrder>>('/api/broker/execute', trade);
  if (!data.success || !data.data) throw new Error(data.error ?? 'Trade execution failed');
  return data.data;
}

export async function cancelOrder(orderId: string): Promise<void> {
  const { data } = await client.post<ApiResponse<null>>(`/api/broker/orders/${orderId}/cancel`);
  if (!data.success) throw new Error(data.error ?? 'Cancel failed');
}

export async function closePosition(symbol: string): Promise<void> {
  const { data } = await client.delete<ApiResponse<null>>(`/api/broker/positions/${symbol}`);
  if (!data.success) throw new Error(data.error ?? 'Close position failed');
}
