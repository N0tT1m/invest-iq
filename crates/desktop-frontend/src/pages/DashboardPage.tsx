import { Box, Grid, Typography, Card, CardContent, Alert } from '@mui/material';
import { useAppStore } from '@/store/appStore';
import { useAnalysis } from '@/hooks/useAnalysis';
import { useBars } from '@/hooks/useBars';
import EmptyState from '@/components/EmptyState';
import LoadingOverlay from '@/components/LoadingOverlay';
import SymbolInput from '@/features/analysis/SymbolInput';
import OverallSignalCard from '@/features/analysis/OverallSignalCard';
import EngineCard from '@/features/analysis/EngineCard';
import PriceChart from '@/features/charts/PriceChart';
import RSIChart from '@/features/charts/RSIChart';
import MACDChart from '@/features/charts/MACDChart';

export default function DashboardPage() {
  const { symbol, timeframe, daysBack } = useAppStore();
  const analysis = useAnalysis(symbol, timeframe, daysBack);
  const bars = useBars(symbol, timeframe, daysBack);

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <SymbolInput />

      {!symbol && (
        <EmptyState
          title="Welcome to InvestIQ"
          message="Enter a symbol in the search bar above to begin analysis."
        />
      )}

      {symbol && analysis.isLoading && <LoadingOverlay message={`Analyzing ${symbol}...`} />}

      {analysis.error && (
        <Alert severity="error">{(analysis.error as Error).message}</Alert>
      )}

      {analysis.data && (
        <>
          <OverallSignalCard analysis={analysis.data} />

          {/* Price Chart */}
          <Card>
            <CardContent>
              <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 1 }}>
                Price Chart
              </Typography>
              {bars.isLoading && <LoadingOverlay message="Loading chart data..." />}
              {bars.data && bars.data.bars.length > 0 && (
                <>
                  <PriceChart bars={bars.data.bars} />
                  <Box sx={{ display: 'flex', gap: 1, mt: 1 }}>
                    <Typography variant="caption" sx={{ color: '#667eea' }}>SMA 20</Typography>
                    <Typography variant="caption" sx={{ color: '#f5576c' }}>SMA 50</Typography>
                  </Box>
                </>
              )}
            </CardContent>
          </Card>

          {/* Technical Indicators */}
          {bars.data && bars.data.bars.length > 35 && (
            <Grid container spacing={2}>
              <Grid size={{ xs: 12, md: 6 }}>
                <Card>
                  <CardContent>
                    <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>RSI (14)</Typography>
                    <RSIChart bars={bars.data.bars} />
                  </CardContent>
                </Card>
              </Grid>
              <Grid size={{ xs: 12, md: 6 }}>
                <Card>
                  <CardContent>
                    <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>MACD</Typography>
                    <MACDChart bars={bars.data.bars} />
                  </CardContent>
                </Card>
              </Grid>
            </Grid>
          )}

          {/* Engine Cards */}
          <Grid container spacing={2}>
            <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
              <EngineCard title="Technical" engine={analysis.data.technical} />
            </Grid>
            <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
              <EngineCard title="Fundamental" engine={analysis.data.fundamental} />
            </Grid>
            <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
              <EngineCard title="Quantitative" engine={analysis.data.quantitative} />
            </Grid>
            <Grid size={{ xs: 12, sm: 6, lg: 3 }}>
              <EngineCard title="Sentiment" engine={analysis.data.sentiment} />
            </Grid>
          </Grid>
        </>
      )}
    </Box>
  );
}
