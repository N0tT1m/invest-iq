import axios from 'axios';
import { useAppStore } from '@/store/appStore';

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://localhost:3000';

const client = axios.create({
  baseURL: API_BASE,
  timeout: 30_000,
  headers: { 'Content-Type': 'application/json' },
});

client.interceptors.request.use((config) => {
  const apiKey = useAppStore.getState().apiKey;
  if (apiKey) {
    config.headers['x-api-key'] = apiKey;
  }
  const liveTradingKey = useAppStore.getState().liveTradingKey;
  if (liveTradingKey) {
    config.headers['x-live-trading-key'] = liveTradingKey;
  }
  return config;
});

export default client;
