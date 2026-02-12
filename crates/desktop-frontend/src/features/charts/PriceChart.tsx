import { useRef, useEffect } from 'react';
import { createChart, CandlestickSeries, HistogramSeries, LineSeries, type IChartApi } from 'lightweight-charts';
import { Box } from '@mui/material';
import { CHART_COLORS } from '@/theme/colors';
import type { Bar } from '@/api/types';

interface PriceChartProps {
  bars: Bar[];
  height?: number;
  showVolume?: boolean;
  showSMA?: boolean;
}

function sma(data: { time: number; close: number }[], period: number) {
  const result: { time: number; value: number }[] = [];
  for (let i = period - 1; i < data.length; i++) {
    let sum = 0;
    for (let j = 0; j < period; j++) sum += data[i - j].close;
    result.push({ time: data[i].time, value: sum / period });
  }
  return result;
}

export default function PriceChart({ bars, height = 450, showVolume = true, showSMA = true }: PriceChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);

  useEffect(() => {
    if (!containerRef.current || bars.length === 0) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height,
      layout: { background: { color: 'transparent' }, textColor: '#a0a0a0' },
      grid: {
        vertLines: { color: 'rgba(255,255,255,0.05)' },
        horzLines: { color: 'rgba(255,255,255,0.05)' },
      },
      crosshair: { mode: 0 },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.1)' },
      timeScale: { borderColor: 'rgba(255,255,255,0.1)', timeVisible: true },
    });
    chartRef.current = chart;

    const candleData = bars.map((b) => ({
      time: Math.floor(b.t / 1000) as import('lightweight-charts').UTCTimestamp,
      open: b.o,
      high: b.h,
      low: b.l,
      close: b.c,
    }));

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: CHART_COLORS.bullish,
      downColor: CHART_COLORS.bearish,
      borderUpColor: CHART_COLORS.bullish,
      borderDownColor: CHART_COLORS.bearish,
      wickUpColor: CHART_COLORS.bullish,
      wickDownColor: CHART_COLORS.bearish,
    });
    candleSeries.setData(candleData);

    if (showVolume) {
      const volumeSeries = chart.addSeries(HistogramSeries, {
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      });
      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.8, bottom: 0 },
      });
      volumeSeries.setData(
        bars.map((b) => ({
          time: Math.floor(b.t / 1000) as import('lightweight-charts').UTCTimestamp,
          value: b.v,
          color: b.c >= b.o ? CHART_COLORS.volumeUp + '66' : CHART_COLORS.volumeDown + '66',
        })),
      );
    }

    if (showSMA) {
      const closePrices = bars.map((b) => ({ time: Math.floor(b.t / 1000), close: b.c }));

      const sma20Data = sma(closePrices, 20);
      if (sma20Data.length > 0) {
        const s20 = chart.addSeries(LineSeries, { color: CHART_COLORS.sma20, lineWidth: 1, priceLineVisible: false });
        s20.setData(sma20Data.map((d) => ({ time: d.time as import('lightweight-charts').UTCTimestamp, value: d.value })));
      }

      const sma50Data = sma(closePrices, 50);
      if (sma50Data.length > 0) {
        const s50 = chart.addSeries(LineSeries, { color: CHART_COLORS.sma50, lineWidth: 1, priceLineVisible: false });
        s50.setData(sma50Data.map((d) => ({ time: d.time as import('lightweight-charts').UTCTimestamp, value: d.value })));
      }
    }

    chart.timeScale().fitContent();

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) chart.applyOptions({ width: entry.contentRect.width });
    });
    observer.observe(containerRef.current);

    return () => {
      observer.disconnect();
      chart.remove();
      chartRef.current = null;
    };
  }, [bars, height, showVolume, showSMA]);

  return <Box ref={containerRef} sx={{ width: '100%', height }} />;
}
