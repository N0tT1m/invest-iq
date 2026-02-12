import { Box, Typography } from '@mui/material';
import TradingTabs from '@/features/trading/TradingTabs';

export default function TradingPage() {
  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>Trading</Typography>
      <TradingTabs />
    </Box>
  );
}
