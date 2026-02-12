import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface BankAccount {
  id: string;
  name: string;
  lastFour: string;
  balance: number;
  color: string;
}

export interface TransferRecord {
  id: string;
  date: string;
  totalAmount: number;
  splits: { bankId: string; bankName: string; amount: number; pct: number }[];
}

const DEFAULT_BANK_ACCOUNTS: BankAccount[] = [
  { id: 'pnc', name: 'PNC Bank', lastFour: '4821', balance: 0, color: '#F58220' },
  { id: 'cap1', name: 'Capital One', lastFour: '7135', balance: 0, color: '#D03027' },
];

interface AppState {
  symbol: string;
  timeframe: string;
  daysBack: number;
  apiKey: string;
  liveTradingKey: string;
  sidebarOpen: boolean;
  bankAccounts: BankAccount[];
  transferHistory: TransferRecord[];

  setSymbol: (s: string) => void;
  setTimeframe: (t: string) => void;
  setDaysBack: (d: number) => void;
  setApiKey: (k: string) => void;
  setLiveTradingKey: (k: string) => void;
  toggleSidebar: () => void;
  addTransfer: (t: TransferRecord) => void;
  updateBankBalance: (id: string, amount: number) => void;
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      symbol: '',
      timeframe: '1d',
      daysBack: 365,
      apiKey: '',
      liveTradingKey: '',
      sidebarOpen: true,
      bankAccounts: DEFAULT_BANK_ACCOUNTS,
      transferHistory: [],

      setSymbol: (symbol) => set({ symbol: symbol.toUpperCase() }),
      setTimeframe: (timeframe) => set({ timeframe }),
      setDaysBack: (daysBack) => set({ daysBack }),
      setApiKey: (apiKey) => set({ apiKey }),
      setLiveTradingKey: (liveTradingKey) => set({ liveTradingKey }),
      toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
      addTransfer: (t) =>
        set((s) => ({
          transferHistory: [t, ...s.transferHistory],
          bankAccounts: s.bankAccounts.map((b) => {
            const split = t.splits.find((sp) => sp.bankId === b.id);
            return split ? { ...b, balance: b.balance + split.amount } : b;
          }),
        })),
      updateBankBalance: (id, amount) =>
        set((s) => ({
          bankAccounts: s.bankAccounts.map((b) =>
            b.id === id ? { ...b, balance: amount } : b,
          ),
        })),
    }),
    {
      name: 'investiq-store',
      partialize: (state) => ({
        apiKey: state.apiKey,
        liveTradingKey: state.liveTradingKey,
        timeframe: state.timeframe,
        daysBack: state.daysBack,
        sidebarOpen: state.sidebarOpen,
        bankAccounts: state.bankAccounts,
        transferHistory: state.transferHistory,
      }),
    },
  ),
);
