import { useRef, useEffect } from 'react';
import { createChart, LineSeries, HistogramSeries, type IChartApi } from 'lightweight-charts';
import { Box } from '@mui/material';
import { CHART_COLORS } from '@/theme/colors';
import type { Bar } from '@/api/types';

interface MACDChartProps {
  bars: Bar[];
  height?: number;
}

function ema(data: number[], period: number): number[] {
  const k = 2 / (period + 1);
  const result: number[] = [data[0]];
  for (let i = 1; i < data.length; i++) {
    result.push(data[i] * k + result[i - 1] * (1 - k));
  }
  return result;
}

function computeMACD(bars: Bar[]) {
  const closes = bars.map((b) => b.c);
  const ema12 = ema(closes, 12);
  const ema26 = ema(closes, 26);
  const macdLine = ema12.map((v, i) => v - ema26[i]);
  const signalLine = ema(macdLine.slice(25), 9);

  const result: { time: number; macd: number; signal: number; histogram: number }[] = [];
  for (let i = 0; i < signalLine.length; i++) {
    const idx = i + 25;
    result.push({
      time: Math.floor(bars[idx].t / 1000),
      macd: macdLine[idx],
      signal: signalLine[i],
      histogram: macdLine[idx] - signalLine[i],
    });
  }
  return result;
}

export default function MACDChart({ bars, height = 150 }: MACDChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);

  useEffect(() => {
    if (!containerRef.current || bars.length < 35) return;

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

    const macdData = computeMACD(bars);

    const histSeries = chart.addSeries(HistogramSeries, {
      priceLineVisible: false,
    });
    histSeries.setData(
      macdData.map((d) => ({
        time: d.time as import('lightweight-charts').UTCTimestamp,
        value: d.histogram,
        color: d.histogram >= 0 ? CHART_COLORS.bullish + '88' : CHART_COLORS.bearish + '88',
      })),
    );

    const macdSeries = chart.addSeries(LineSeries, {
      color: CHART_COLORS.macdLine,
      lineWidth: 2,
      priceLineVisible: false,
    });
    macdSeries.setData(
      macdData.map((d) => ({ time: d.time as import('lightweight-charts').UTCTimestamp, value: d.macd })),
    );

    const signalSeries = chart.addSeries(LineSeries, {
      color: CHART_COLORS.macdSignal,
      lineWidth: 1,
      priceLineVisible: false,
    });
    signalSeries.setData(
      macdData.map((d) => ({ time: d.time as import('lightweight-charts').UTCTimestamp, value: d.signal })),
    );

    chart.timeScale().fitContent();

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) chart.applyOptions({ width: entry.contentRect.width });
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      chart.remove();
    };
  }, [bars, height]);

  return <Box ref={containerRef} sx={{ width: '100%', height }} />;
}
