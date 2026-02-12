import { useState } from 'react';
import {
  Box, Grid, Card, CardContent, Typography, TextField, Button,
  FormControl, InputLabel, Select, MenuItem, Alert,
} from '@mui/material';
import MetricCard from '@/components/MetricCard';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useAccount, useBrokerPositions, useExecuteTrade } from '@/hooks/useBroker';
import { useAppStore } from '@/store/appStore';
import { safeFloat, formatCurrency, formatPercent } from '@/utils/format';

export default function PaperTrading() {
  const symbol = useAppStore((s) => s.symbol);
  const account = useAccount();
  const positions = useBrokerPositions();
  const executeTrade = useExecuteTrade();

  const [side, setSide] = useState<'buy' | 'sell'>('buy');
  const [qty, setQty] = useState('1');
  const [orderType, setOrderType] = useState('market');

  if (account.isLoading) return <LoadingOverlay message="Loading account..." />;

  const acct = account.data;

  const handleTrade = () => {
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
      {account.error && <Alert severity="error">{(account.error as Error).message}</Alert>}

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
            <MetricCard title="Portfolio Value" value={formatCurrency(safeFloat(acct.portfolio_value))} />
          </Grid>
        </Grid>
      )}

      {/* Trade Form */}
      <Card>
        <CardContent>
          <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Place Paper Trade</Typography>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
            <Typography variant="body1" sx={{ fontWeight: 600, minWidth: 60 }}>
              {symbol || 'No symbol'}
            </Typography>
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
              color={side === 'buy' ? 'success' : 'error'}
              onClick={handleTrade}
              disabled={!symbol || executeTrade.isPending}
            >
              {executeTrade.isPending ? 'Submitting...' : `${side.toUpperCase()} ${symbol}`}
            </Button>
          </Box>
          {executeTrade.isSuccess && <Alert severity="success" sx={{ mt: 2 }}>Order placed successfully</Alert>}
          {executeTrade.error && <Alert severity="error" sx={{ mt: 2 }}>{(executeTrade.error as Error).message}</Alert>}
        </CardContent>
      </Card>

      {/* Positions */}
      {positions.data && positions.data.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Open Positions</Typography>
            {positions.data.map((pos) => (
              <Box
                key={pos.symbol}
                sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', py: 1, borderBottom: '1px solid', borderColor: 'divider' }}
              >
                <Box>
                  <Typography variant="body2" sx={{ fontWeight: 600 }}>{pos.symbol}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    {pos.qty} shares @ {formatCurrency(safeFloat(pos.avg_entry_price))}
                  </Typography>
                </Box>
                <Box sx={{ textAlign: 'right' }}>
                  <Typography variant="body2">{formatCurrency(safeFloat(pos.market_value))}</Typography>
                  <Typography
                    variant="caption"
                    sx={{ color: safeFloat(pos.unrealized_pl) >= 0 ? '#00cc88' : '#ff4444' }}
                  >
                    {formatCurrency(safeFloat(pos.unrealized_pl))} ({formatPercent(safeFloat(pos.unrealized_plpc) * 100)})
                  </Typography>
                </Box>
              </Box>
            ))}
          </CardContent>
        </Card>
      )}
    </Box>
  );
}
