import { useRef, useEffect } from 'react';
import { createChart, LineSeries, type IChartApi } from 'lightweight-charts';
import { Box } from '@mui/material';
import { CHART_COLORS } from '@/theme/colors';
import type { Bar } from '@/api/types';

interface RSIChartProps {
  bars: Bar[];
  period?: number;
  height?: number;
}

function computeRSI(bars: Bar[], period = 14) {
  const closes = bars.map((b) => b.c);
  const rsiData: { time: number; value: number }[] = [];
  let gains = 0;
  let losses = 0;

  for (let i = 1; i <= period && i < closes.length; i++) {
    const diff = closes[i] - closes[i - 1];
    if (diff > 0) gains += diff;
    else losses -= diff;
  }

  let avgGain = gains / period;
  let avgLoss = losses / period;

  for (let i = period; i < closes.length; i++) {
    if (i > period) {
      const diff = closes[i] - closes[i - 1];
      avgGain = (avgGain * (period - 1) + Math.max(diff, 0)) / period;
      avgLoss = (avgLoss * (period - 1) + Math.max(-diff, 0)) / period;
    }
    const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
    rsiData.push({ time: Math.floor(bars[i].t / 1000), value: 100 - 100 / (1 + rs) });
  }

  return rsiData;
}

export default function RSIChart({ bars, period = 14, height = 150 }: RSIChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);

  useEffect(() => {
    if (!containerRef.current || bars.length < period + 1) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height,
      layout: { background: { color: 'transparent' }, textColor: '#a0a0a0' },
      grid: {
        vertLines: { color: 'rgba(255,255,255,0.05)' },
        horzLines: { color: 'rgba(255,255,255,0.05)' },
      },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.1)' },
      timeScale: { borderColor: 'rgba(255,255,255,0.1)', visible: false },
    });
    chartRef.current = chart;

    const rsiData = computeRSI(bars, period);
    const series = chart.addSeries(LineSeries, {
      color: CHART_COLORS.rsi,
      lineWidth: 2,
      priceLineVisible: false,
    });
    series.setData(rsiData.map((d) => ({
      time: d.time as import('lightweight-charts').UTCTimestamp,
      value: d.value,
    })));

    // Overbought/oversold lines
    series.createPriceLine({ price: 70, color: '#ff444488', lineWidth: 1, lineStyle: 2, axisLabelVisible: false });
    series.createPriceLine({ price: 30, color: '#38ef7d88', lineWidth: 1, lineStyle: 2, axisLabelVisible: false });

    chart.timeScale().fitContent();

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) chart.applyOptions({ width: entry.contentRect.width });
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      chart.remove();
    };
  }, [bars, period, height]);

  return <Box ref={containerRef} sx={{ width: '100%', height }} />;
}
