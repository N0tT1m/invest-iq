// Signal colors matching Python frontend
export const SIGNAL_COLORS = {
  strongBuy: '#00cc88',
  buy: '#00ccff',
  neutral: '#888888',
  sell: '#ffaa00',
  strongSell: '#ff4444',
} as const;

// Chart colors
export const CHART_COLORS = {
  bullish: '#38ef7d',
  bearish: '#f45c43',
  bullishFill: 'rgba(56, 239, 125, 0.3)',
  bearishFill: 'rgba(244, 92, 67, 0.3)',
  sma20: '#667eea',
  sma50: '#f5576c',
  sma200: '#00f2fe',
  rsi: '#00f2fe',
  macdLine: '#667eea',
  macdSignal: '#f5576c',
  bollinger: '#667eea',
  bollingerFill: 'rgba(102, 126, 234, 0.3)',
  volumeUp: '#38ef7d',
  volumeDown: '#f45c43',
} as const;

// Sentiment colors
export const SENTIMENT_COLORS = {
  positive: '#00cc96',
  negative: '#ef553b',
  neutral: '#636efa',
} as const;

// Portfolio colors
export const PORTFOLIO_COLORS = {
  gain: '#00CC96',
  loss: '#EF553B',
  default: '#636EFA',
  accent: '#FFA15A',
} as const;

// UI gradient presets
export const GRADIENTS = {
  primary: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
  success: 'linear-gradient(135deg, #11998e 0%, #38ef7d 100%)',
  danger: 'linear-gradient(135deg, #eb3349 0%, #f45c43 100%)',
  warning: 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)',
  info: 'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)',
} as const;

// Series of colors for multi-series charts
export const SERIES_PALETTE = [
  '#667eea', '#f5576c', '#00f2fe', '#38ef7d', '#FFA15A',
  '#764ba2', '#11998e', '#eb3349', '#4facfe', '#f093fb',
] as const;
