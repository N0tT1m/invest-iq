import { Box, Typography, Grid } from '@mui/material';
import { useAppStore } from '@/store/appStore';
import SocialSentiment from '@/features/intelligence/SocialSentiment';
import MacroOverlay from '@/features/intelligence/MacroOverlay';
import SmartWatchlist from '@/features/intelligence/SmartWatchlist';
import FlowMap from '@/features/intelligence/FlowMap';
import AlphaDecay from '@/features/intelligence/AlphaDecay';
import TaxDashboard from '@/features/intelligence/TaxDashboard';

export default function IntelligencePage() {
  const symbol = useAppStore((s) => s.symbol);

  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>Market Intelligence</Typography>
      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
        <Grid container spacing={2}>
          <Grid size={{ xs: 12, md: 6 }}>
            <MacroOverlay />
          </Grid>
          <Grid size={{ xs: 12, md: 6 }}>
            {symbol && <SocialSentiment symbol={symbol} />}
          </Grid>
        </Grid>

        <FlowMap />
        <SmartWatchlist />
        <AlphaDecay />
        <TaxDashboard />
      </Box>
    </Box>
  );
}
