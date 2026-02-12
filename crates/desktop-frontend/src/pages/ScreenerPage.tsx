import { Box, Typography } from '@mui/material';
import StockScreener from '@/features/search/StockScreener';

export default function ScreenerPage() {
  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>Stock Screener</Typography>
      <StockScreener />
    </Box>
  );
}
