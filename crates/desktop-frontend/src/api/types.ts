// Generic API response wrapper
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// ---- Core Analysis ----
export interface AnalysisResult {
  symbol: string;
  overall_signal: string;
  overall_confidence: number;
  current_price?: number;
  technical: EngineResult;
  fundamental: EngineResult;
  quantitative: EngineResult;
  sentiment: EngineResult;
  risk_score?: number;
  risk_level?: string;
  timestamp?: string;
}

export interface EngineResult {
  signal: string;
  confidence: number;
  details: Record<string, unknown>;
}

// ---- Bars / OHLCV ----
export interface Bar {
  t: number; // unix timestamp ms
  o: number;
  h: number;
  l: number;
  c: number;
  v: number;
}

export interface BarsResponse {
  symbol: string;
  bars: Bar[];
  timeframe?: string;
}

// ---- Broker ----
export interface BrokerAccount {
  id: string;
  account_number?: string;
  status: string;
  currency: string;
  cash: string;
  portfolio_value: string;
  buying_power: string;
  equity: string;
  last_equity?: string;
  long_market_value?: string;
  short_market_value?: string;
  pattern_day_trader?: boolean;
  daytrade_count?: number;
}

export interface BrokerPosition {
  symbol: string;
  qty: string;
  side: string;
  market_value: string;
  cost_basis: string;
  unrealized_pl: string;
  unrealized_plpc: string;
  current_price: string;
  avg_entry_price: string;
  change_today?: string;
}

export interface BrokerOrder {
  id: string;
  symbol: string;
  side: string;
  qty: string;
  type: string;
  time_in_force: string;
  status: string;
  filled_avg_price?: string;
  filled_qty?: string;
  created_at: string;
  updated_at?: string;
  limit_price?: string;
  stop_price?: string;
}

export interface TradeRequest {
  symbol: string;
  side: 'buy' | 'sell';
  qty: number;
  order_type?: string;
  time_in_force?: string;
  limit_price?: number;
  stop_price?: number;
}

// ---- Backtest ----
export interface BacktestResult {
  id?: string;
  symbol: string;
  strategy?: string;
  total_return: number;
  sharpe_ratio: number;
  max_drawdown: number;
  win_rate: number;
  total_trades: number;
  profit_factor?: number;
  avg_win?: number;
  avg_loss?: number;
  equity_curve?: EquityPoint[];
  trades?: BacktestTrade[];
}

export interface EquityPoint {
  date: string;
  equity: number;
}

export interface BacktestTrade {
  entry_date: string;
  exit_date: string;
  side: string;
  entry_price: number;
  exit_price: number;
  pnl: number;
  pnl_pct: number;
}

// ---- Agent Trades ----
export interface PendingTrade {
  id: string;
  symbol: string;
  side: string;
  qty: number;
  signal: string;
  confidence: number;
  regime?: string;
  reasoning?: string;
  status: string;
  created_at: string;
}

export interface AgentAnalyticsSummary {
  total_trades: number;
  approved: number;
  rejected: number;
  win_rate?: number;
  total_pnl?: number;
  avg_confidence?: number;
}

// ---- Sentiment ----
export interface SentimentVelocity {
  symbol: string;
  velocity: number;
  acceleration: number;
  current_sentiment: number;
  trend: string;
  history?: SentimentPoint[];
}

export interface SentimentPoint {
  timestamp: string;
  score: number;
}

export interface SocialSentiment {
  symbol: string;
  overall_score: number;
  sentiment_label: string;
  components: Record<string, number>;
  data_source?: string;
}

// ---- Risk ----
export interface RiskRadar {
  symbol?: string;
  dimensions: RiskDimension[];
  overall_risk: number;
  risk_level: string;
}

export interface RiskDimension {
  name: string;
  score: number;
  weight: number;
  details?: string;
}

// ---- Calibration ----
export interface CalibrationResult {
  raw_confidence: number;
  calibrated_confidence: number;
  uncertainty: number;
  components?: Record<string, number>;
}

// ---- Strategy Health / Alpha Decay ----
export interface StrategyHealth {
  name: string;
  status: string;
  sharpe_ratio: number;
  win_rate: number;
  decay_rate?: number;
  last_updated?: string;
}

// ---- Watchlist ----
export interface WatchlistItem {
  symbol: string;
  signal?: string;
  score?: number;
  price?: number;
  change_pct?: number;
}

// ---- Market Flows ----
export interface SectorFlow {
  sector: string;
  etf: string;
  change_pct: number;
  volume: number;
  trend: string;
}

// ---- Macro ----
export interface MacroIndicators {
  regime: string;
  trend: string;
  rates_direction: string;
  volatility: string;
  indicators: Record<string, unknown>;
}

// ---- Earnings / Dividends / Options / Insider ----
export interface EarningsData {
  symbol: string;
  eps_growth?: number;
  revenue_growth?: number;
  history: Record<string, unknown>[];
  data_source?: string;
}

export interface DividendData {
  symbol: string;
  yield_pct?: number;
  frequency?: string;
  growth_rate?: number;
  history: Record<string, unknown>[];
  data_source?: string;
}

export interface OptionsData {
  symbol: string;
  put_call_ratio?: number;
  implied_volatility?: number;
  flows: Record<string, unknown>[];
  data_source?: string;
}

export interface ShortInterestData {
  symbol: string;
  short_interest_score: number;
  components: Record<string, number>;
  interpretation?: string;
  data_source?: string;
}

export interface InsiderData {
  symbol: string;
  transactions: Record<string, unknown>[];
  net_sentiment?: string;
  data_source?: string;
}

export interface CorrelationData {
  symbol: string;
  benchmarks: Record<string, number>;
  rolling_correlation?: { date: string; value: number }[];
}

// ---- ML ----
export interface MLTradeSignal {
  symbol: string;
  probability: number;
  signal: string;
  features: Record<string, number>;
}

export interface MLSentimentResult {
  symbol: string;
  score: number;
  label: string;
  distribution: { positive: number; negative: number; neutral: number };
}

export interface MLPriceForecast {
  symbol: string;
  forecast: { date: string; price: number; lower: number; upper: number }[];
  model?: string;
}

export interface MLCalibrationResult {
  symbol: string;
  raw: number;
  calibrated: number;
  components?: Record<string, number>;
}

export interface MLStrategyWeights {
  weights: { engine: string; weight: number; credible_low?: number; credible_high?: number }[];
}

// ---- Symbol Search / Screener ----
export interface SymbolSearchResult {
  symbol: string;
  name: string;
  type?: string;
  exchange?: string;
}

export interface ScreenerResult {
  symbol: string;
  name?: string;
  price?: number;
  change_pct?: number;
  volume?: number;
  signal?: string;
  score?: number;
}

// ---- Portfolio ----
export interface PortfolioSummary {
  total_value: number;
  total_pnl: number;
  total_pnl_pct: number;
  positions: BrokerPosition[];
}

// ---- Tax ----
export interface TaxLot {
  id: string;
  symbol: string;
  qty: number;
  cost_basis: number;
  acquired_date: string;
  unrealized_pnl?: number;
  holding_period?: string;
}

export interface TaxYearEndSummary {
  year: number;
  short_term_gains: number;
  long_term_gains: number;
  total_gains: number;
  wash_sale_adjustments: number;
  harvested_losses: number;
}
