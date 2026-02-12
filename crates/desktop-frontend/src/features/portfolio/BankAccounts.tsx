import { Box, Card, CardContent, Grid, Typography } from '@mui/material';
import AccountBalanceIcon from '@mui/icons-material/AccountBalance';
import { useAppStore } from '@/store/appStore';
import { formatCurrency } from '@/utils/format';

export default function BankAccounts() {
  const bankAccounts = useAppStore((s) => s.bankAccounts);

  return (
    <Box>
      <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>
        Linked Bank Accounts
      </Typography>
      <Grid container spacing={2}>
        {bankAccounts.map((bank) => (
          <Grid key={bank.id} size={{ xs: 12, sm: 6 }}>
            <Card
              sx={{
                borderLeft: `4px solid ${bank.color}`,
                height: '100%',
              }}
            >
              <CardContent>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 1.5 }}>
                  <AccountBalanceIcon sx={{ color: bank.color, fontSize: 28 }} />
                  <Box>
                    <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
                      {bank.name}
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      ****{bank.lastFour}
                    </Typography>
                  </Box>
                </Box>
                <Typography variant="h5" sx={{ fontWeight: 700 }}>
                  {formatCurrency(bank.balance)}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  Transferred total
                </Typography>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
