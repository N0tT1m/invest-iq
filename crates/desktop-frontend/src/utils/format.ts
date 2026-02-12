/** Safely convert any value to a number, returning fallback on failure */
export function safeFloat(val: unknown, fallback = 0): number {
  if (val == null) return fallback;
  const n = typeof val === 'number' ? val : Number(val);
  return Number.isFinite(n) ? n : fallback;
}

export function formatCurrency(val: unknown, decimals = 2): string {
  const n = safeFloat(val);
  return n.toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

export function formatPercent(val: unknown, decimals = 2): string {
  const n = safeFloat(val);
  return `${n >= 0 ? '+' : ''}${n.toFixed(decimals)}%`;
}

export function formatNumber(val: unknown, decimals = 2): string {
  const n = safeFloat(val);
  return n.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

export function formatCompact(val: unknown): string {
  const n = safeFloat(val);
  if (Math.abs(n) >= 1e9) return `${(n / 1e9).toFixed(1)}B`;
  if (Math.abs(n) >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (Math.abs(n) >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return n.toFixed(0);
}

/** Map signal string to display color */
export function signalColor(signal: string | undefined): string {
  switch (signal?.toLowerCase()) {
    case 'strong_buy':
    case 'strongbuy':
      return '#00cc88';
    case 'buy':
      return '#00ccff';
    case 'sell':
      return '#ffaa00';
    case 'strong_sell':
    case 'strongsell':
      return '#ff4444';
    default:
      return '#888888';
  }
}
