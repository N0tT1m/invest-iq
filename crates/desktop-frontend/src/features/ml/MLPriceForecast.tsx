import { useRef, useEffect } from 'react';
import { createChart, LineSeries, AreaSeries, type IChartApi } from 'lightweight-charts';
import { Card, CardContent, Typography, Box, Chip, Alert } from '@mui/material';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMLPriceForecast } from '@/hooks/useML';

interface Props { symbol: string }

export default function MLPriceForecast({ symbol }: Props) {
  const { data, isLoading, error } = useMLPriceForecast(symbol);
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);

  useEffect(() => {
    if (!containerRef.current || !data || data.forecast.length === 0) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 280,
      layout: { background: { color: 'transparent' }, textColor: '#a0a0a0' },
      grid: { vertLines: { color: 'rgba(255,255,255,0.05)' }, horzLines: { color: 'rgba(255,255,255,0.05)' } },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.1)' },
      timeScale: { borderColor: 'rgba(255,255,255,0.1)' },
    });
    chartRef.current = chart;

    type TS = import('lightweight-charts').UTCTimestamp;

    // Upper confidence band
    const upperSeries = chart.addSeries(AreaSeries, {
      lineColor: 'rgba(102, 126, 234, 0.3)',
      topColor: 'rgba(102, 126, 234, 0.15)',
      bottomColor: 'rgba(102, 126, 234, 0.0)',
      lineWidth: 1,
      priceLineVisible: false,
    });
    upperSeries.setData(
      data.forecast.map((d) => ({ time: d.date as unknown as TS, value: d.upper })),
    );

    // Lower confidence band
    const lowerSeries = chart.addSeries(LineSeries, {
      color: 'rgba(102, 126, 234, 0.3)',
      lineWidth: 1,
      priceLineVisible: false,
    });
    lowerSeries.setData(
      data.forecast.map((d) => ({ time: d.date as unknown as TS, value: d.lower })),
    );

    // Forecast line
    const forecastSeries = chart.addSeries(LineSeries, {
      color: '#667eea',
      lineWidth: 2,
      priceLineVisible: false,
    });
    forecastSeries.setData(
      data.forecast.map((d) => ({ time: d.date as unknown as TS, value: d.price })),
    );

    chart.timeScale().fitContent();

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) chart.applyOptions({ width: entry.contentRect.width });
    });
    observer.observe(containerRef.current);

    return () => { observer.disconnect(); chart.remove(); };
  }, [data]);

  if (isLoading) return <LoadingOverlay message="Loading forecast..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>ML Price Forecast</Typography>
          {data.model && <Chip label={data.model} size="small" variant="outlined" />}
        </Box>
        <Box ref={containerRef} sx={{ width: '100%', height: 280 }} />
      </CardContent>
    </Card>
  );
}
