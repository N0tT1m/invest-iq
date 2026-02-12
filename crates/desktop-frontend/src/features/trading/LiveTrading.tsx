import { useState } from 'react';
import {
  Box, Card, CardContent, Typography, TextField, Button,
  FormControl, InputLabel, Select, MenuItem, Alert, Grid,
} from '@mui/material';
import { Warning as WarningIcon } from '@mui/icons-material';
import MetricCard from '@/components/MetricCard';
import ConfirmDialog from '@/components/ConfirmDialog';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useAccount, useExecuteTrade } from '@/hooks/useBroker';
import { useAppStore } from '@/store/appStore';
import { safeFloat, formatCurrency } from '@/utils/format';

export default function LiveTrading() {
  const symbol = useAppStore((s) => s.symbol);
  const account = useAccount();
  const executeTrade = useExecuteTrade();

  const [side, setSide] = useState<'buy' | 'sell'>('buy');
  const [qty, setQty] = useState('1');
  const [orderType, setOrderType] = useState('market');
  const [confirmOpen, setConfirmOpen] = useState(false);

  if (account.isLoading) return <LoadingOverlay message="Loading account..." />;

  const acct = account.data;

  const handleConfirm = () => {
    setConfirmOpen(false);
    if (!symbol) return;
    executeTrade.mutate({
      symbol,
      side,
      qty: Number(qty),
      order_type: orderType,
      time_in_force: 'day',
    });
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <Alert severity="warning" icon={<WarningIcon />}>
        Live Trading uses real money. All orders are executed against your connected brokerage account.
      </Alert>

      {acct && (
        <Grid container spacing={2}>
          <Grid size={{ xs: 6, md: 3 }}>
            <MetricCard title="Equity" value={formatCurrency(safeFloat(acct.equity))} />
          </Grid>
          <Grid size={{ xs: 6, md: 3 }}>
            <MetricCard title="Cash" value={formatCurrency(safeFloat(acct.cash))} />
          </Grid>
          <Grid size={{ xs: 6, md: 3 }}>
            <MetricCard title="Buying Power" value={formatCurrency(safeFloat(acct.buying_power))} />
          </Grid>
          <Grid size={{ xs: 6, md: 3 }}>
            <MetricCard title="Day Trades" value={acct.daytrade_count ?? 0} />
          </Grid>
        </Grid>
      )}

      <Card sx={{ border: '1px solid', borderColor: 'warning.main' }}>
        <CardContent>
          <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2, color: 'warning.main' }}>
            Place Live Order
          </Typography>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
            <Typography variant="body1" sx={{ fontWeight: 600 }}>{symbol || 'No symbol'}</Typography>
            <FormControl size="small" sx={{ minWidth: 90 }}>
              <InputLabel>Side</InputLabel>
              <Select value={side} label="Side" onChange={(e) => setSide(e.target.value as 'buy' | 'sell')}>
                <MenuItem value="buy">Buy</MenuItem>
                <MenuItem value="sell">Sell</MenuItem>
              </Select>
            </FormControl>
            <TextField size="small" label="Qty" type="number" value={qty} onChange={(e) => setQty(e.target.value)} sx={{ width: 80 }} />
            <FormControl size="small" sx={{ minWidth: 100 }}>
              <InputLabel>Type</InputLabel>
              <Select value={orderType} label="Type" onChange={(e) => setOrderType(e.target.value)}>
                <MenuItem value="market">Market</MenuItem>
                <MenuItem value="limit">Limit</MenuItem>
              </Select>
            </FormControl>
            <Button
              variant="contained"
              color="warning"
              onClick={() => setConfirmOpen(true)}
              disabled={!symbol || executeTrade.isPending}
            >
              {executeTrade.isPending ? 'Submitting...' : `LIVE ${side.toUpperCase()}`}
            </Button>
          </Box>
          {executeTrade.isSuccess && <Alert severity="success" sx={{ mt: 2 }}>Live order placed successfully</Alert>}
          {executeTrade.error && <Alert severity="error" sx={{ mt: 2 }}>{(executeTrade.error as Error).message}</Alert>}
        </CardContent>
      </Card>

      <ConfirmDialog
        open={confirmOpen}
        title="Confirm Live Trade"
        message={`You are about to ${side} ${qty} shares of ${symbol} with real money. This action cannot be undone.`}
        confirmLabel="Execute Trade"
        confirmColor="warning"
        onConfirm={handleConfirm}
        onCancel={() => setConfirmOpen(false)}
      />
    </Box>
  );
}
