import { Box, Typography, Grid } from '@mui/material';
import { useAppStore } from '@/store/appStore';
import EmptyState from '@/components/EmptyState';
import EarningsPanel from '@/features/research/EarningsPanel';
import DividendPanel from '@/features/research/DividendPanel';
import OptionsFlow from '@/features/research/OptionsFlow';
import ShortInterest from '@/features/research/ShortInterest';
import InsiderActivity from '@/features/research/InsiderActivity';
import CorrelationMatrix from '@/features/research/CorrelationMatrix';

export default function ResearchPage() {
  const symbol = useAppStore((s) => s.symbol);

  if (!symbol) return <EmptyState title="Research" message="Enter a symbol to view research panels." />;

  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>Research: {symbol}</Typography>
      <Grid container spacing={2}>
        <Grid size={{ xs: 12, md: 6 }}>
          <EarningsPanel symbol={symbol} />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <DividendPanel symbol={symbol} />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <OptionsFlow symbol={symbol} />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <ShortInterest symbol={symbol} />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <InsiderActivity symbol={symbol} />
        </Grid>
        <Grid size={{ xs: 12, md: 6 }}>
          <CorrelationMatrix symbol={symbol} />
        </Grid>
      </Grid>
    </Box>
  );
}
