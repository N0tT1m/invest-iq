import { Box, Typography, Grid } from '@mui/material';
import { useAppStore } from '@/store/appStore';
import EmptyState from '@/components/EmptyState';
import MLTradeSignal from '@/features/ml/MLTradeSignal';
import MLSentiment from '@/features/ml/MLSentiment';
import MLPriceForecast from '@/features/ml/MLPriceForecast';
import MLCalibration from '@/features/ml/MLCalibration';
import MLStrategyWeights from '@/features/ml/MLStrategyWeights';

export default function MLPage() {
  const symbol = useAppStore((s) => s.symbol);

  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>ML Insights</Typography>

      {!symbol ? (
        <EmptyState title="ML Insights" message="Enter a symbol to view ML-powered analysis." />
      ) : (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
          <Grid container spacing={2}>
            <Grid size={{ xs: 12, md: 6 }}>
              <MLTradeSignal symbol={symbol} />
            </Grid>
            <Grid size={{ xs: 12, md: 6 }}>
              <MLSentiment symbol={symbol} />
            </Grid>
          </Grid>

          <MLPriceForecast symbol={symbol} />

          <Grid container spacing={2}>
            <Grid size={{ xs: 12, md: 6 }}>
              <MLCalibration symbol={symbol} />
            </Grid>
            <Grid size={{ xs: 12, md: 6 }}>
              <MLStrategyWeights />
            </Grid>
          </Grid>
        </Box>
      )}
    </Box>
  );
}
