import { Box, Typography } from '@mui/material';
import PortfolioDashboard from '@/features/trading/PortfolioDashboard';
import BankAccounts from '@/features/portfolio/BankAccounts';
import TransferFunds from '@/features/portfolio/TransferFunds';

export default function PortfolioPage() {
  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <Typography variant="h5" sx={{ fontWeight: 700 }}>Portfolio</Typography>
      <PortfolioDashboard />
      <BankAccounts />
      <TransferFunds />
    </Box>
  );
}
