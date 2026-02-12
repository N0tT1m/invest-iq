import { useRef, useEffect } from 'react';
import { Box, Grid, Card, CardContent, Typography, Alert } from '@mui/material';
import { createChart, AreaSeries, type IChartApi } from 'lightweight-charts';
import MetricCard from '@/components/MetricCard';
import DataTable from '@/components/DataTable';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useBacktest } from '@/hooks/useBacktest';
import { useAppStore } from '@/store/appStore';
import { formatPercent, formatCurrency } from '@/utils/format';
import type { BacktestTrade } from '@/api/types';

function EquityCurve({ data }: { data: { date: string; equity: number }[] }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);

  useEffect(() => {
    if (!containerRef.current || data.length === 0) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 300,
      layout: { background: { color: 'transparent' }, textColor: '#a0a0a0' },
      grid: { vertLines: { color: 'rgba(255,255,255,0.05)' }, horzLines: { color: 'rgba(255,255,255,0.05)' } },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.1)' },
      timeScale: { borderColor: 'rgba(255,255,255,0.1)' },
    });
    chartRef.current = chart;

    const series = chart.addSeries(AreaSeries, {
      lineColor: '#667eea',
      topColor: 'rgba(102, 126, 234, 0.4)',
      bottomColor: 'rgba(102, 126, 234, 0.0)',
      lineWidth: 2,
    });
    series.setData(
      data.map((d) => ({
        time: d.date as unknown as import('lightweight-charts').UTCTimestamp,
        value: d.equity,
      })),
    );
    chart.timeScale().fitContent();

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) chart.applyOptions({ width: entry.contentRect.width });
    });
    observer.observe(containerRef.current);

    return () => { observer.disconnect(); chart.remove(); };
  }, [data]);

  return <Box ref={containerRef} sx={{ width: '100%', height: 300 }} />;
}

export default function BacktestPanel() {
  const { symbol, daysBack } = useAppStore();
  const { data, isLoading, error } = useBacktest(symbol, daysBack);

  if (!symbol) return <Typography color="text.secondary">Select a symbol to run a backtest.</Typography>;
  if (isLoading) return <LoadingOverlay message={`Running backtest for ${symbol}...`} />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const tradeColumns = [
    { key: 'entry_date', header: 'Entry', render: (r: BacktestTrade) => r.entry_date.slice(0, 10) },
    { key: 'exit_date', header: 'Exit', render: (r: BacktestTrade) => r.exit_date.slice(0, 10) },
    { key: 'side', header: 'Side' },
    { key: 'entry_price', header: 'Entry $', align: 'right' as const, render: (r: BacktestTrade) => formatCurrency(r.entry_price) },
    { key: 'exit_price', header: 'Exit $', align: 'right' as const, render: (r: BacktestTrade) => formatCurrency(r.exit_price) },
    {
      key: 'pnl', header: 'P&L', align: 'right' as const,
      render: (r: BacktestTrade) => (
        <Typography variant="body2" sx={{ color: r.pnl >= 0 ? '#00cc88' : '#ff4444' }}>
          {formatCurrency(r.pnl)} ({formatPercent(r.pnl_pct * 100)})
        </Typography>
      ),
    },
  ];

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <Grid container spacing={2}>
        <Grid size={{ xs: 6, md: 3 }}>
          <MetricCard title="Total Return" value={formatPercent(data.total_return * 100)} color={data.total_return >= 0 ? '#00cc88' : '#ff4444'} />
        </Grid>
        <Grid size={{ xs: 6, md: 3 }}>
          <MetricCard title="Sharpe Ratio" value={data.sharpe_ratio.toFixed(2)} />
        </Grid>
        <Grid size={{ xs: 6, md: 3 }}>
          <MetricCard title="Max Drawdown" value={formatPercent(data.max_drawdown * 100)} color="#ff4444" />
        </Grid>
        <Grid size={{ xs: 6, md: 3 }}>
          <MetricCard title="Win Rate" value={formatPercent(data.win_rate * 100)} subtitle={`${data.total_trades} trades`} />
        </Grid>
      </Grid>

      {data.equity_curve && data.equity_curve.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 1 }}>Equity Curve</Typography>
            <EquityCurve data={data.equity_curve} />
          </CardContent>
        </Card>
      )}

      {data.trades && data.trades.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Trade History</Typography>
            <DataTable columns={tradeColumns} data={data.trades} />
          </CardContent>
        </Card>
      )}
    </Box>
  );
}
