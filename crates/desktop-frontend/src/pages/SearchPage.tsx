import { Box, Typography } from '@mui/material';
import SymbolSearch from '@/features/search/SymbolSearch';

export default function SearchPage() {
  return (
    <Box>
      <Typography variant="h5" sx={{ fontWeight: 700, mb: 3 }}>Symbol Search</Typography>
      <SymbolSearch />
    </Box>
  );
}
