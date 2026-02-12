import { Box, Grid, Card, CardContent, Typography, Alert } from '@mui/material';
import { PieChart, Pie, Cell, BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, ReferenceLine } from 'recharts';
import MetricCard from '@/components/MetricCard';
import DataTable from '@/components/DataTable';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useBrokerPositions, useAccount, useOrders } from '@/hooks/useBroker';
import { safeFloat, formatCurrency, formatPercent } from '@/utils/format';
import { SERIES_PALETTE, PORTFOLIO_COLORS } from '@/theme/colors';
import type { BrokerPosition, BrokerOrder } from '@/api/types';

export default function PortfolioDashboard() {
  const account = useAccount();
  const positions = useBrokerPositions();
  const orders = useOrders();

  if (account.isLoading || positions.isLoading) return <LoadingOverlay message="Loading portfolio..." />;
  if (account.error) return <Alert severity="error">{(account.error as Error).message}</Alert>;

  const posData = positions.data ?? [];
  const allocationData = posData.map((p) => ({
    name: p.symbol,
    value: Math.abs(safeFloat(p.market_value)),
  }));

  const totalUnrealizedPL = posData.reduce((sum, p) => sum + safeFloat(p.unrealized_pl), 0);
  const totalCostBasis = posData.reduce((sum, p) => sum + safeFloat(p.cost_basis), 0);
  const totalPLPct = totalCostBasis > 0 ? (totalUnrealizedPL / totalCostBasis) * 100 : 0;

  const equity = safeFloat(account.data?.equity);
  const lastEquity = safeFloat(account.data?.last_equity);
  const equityChange = lastEquity > 0 ? equity - lastEquity : 0;
  const equityChangePct = lastEquity > 0 ? (equityChange / lastEquity) * 100 : 0;

  const plBarData = posData
    .map((p) => ({
      symbol: p.symbol,
      pl: safeFloat(p.unrealized_pl),
      fill: safeFloat(p.unrealized_pl) >= 0 ? PORTFOLIO_COLORS.gain : PORTFOLIO_COLORS.loss,
    }))
    .sort((a, b) => b.pl - a.pl);

  const posColumns = [
    { key: 'symbol', header: 'Symbol', render: (r: BrokerPosition) => <Typography variant="body2" fontWeight={600}>{r.symbol}</Typography> },
    { key: 'qty', header: 'Qty', align: 'right' as const },
    { key: 'current_price', header: 'Price', align: 'right' as const, render: (r: BrokerPosition) => formatCurrency(safeFloat(r.current_price)) },
    { key: 'market_value', header: 'Value', align: 'right' as const, render: (r: BrokerPosition) => formatCurrency(safeFloat(r.market_value)) },
    {
      key: 'unrealized_pl', header: 'P&L', align: 'right' as const,
      render: (r: BrokerPosition) => {
        const pl = safeFloat(r.unrealized_pl);
        return <Typography variant="body2" sx={{ color: pl >= 0 ? '#00cc88' : '#ff4444' }}>{formatCurrency(pl)}</Typography>;
      },
    },
    {
      key: 'unrealized_plpc', header: '%', align: 'right' as const,
      render: (r: BrokerPosition) => {
        const pct = safeFloat(r.unrealized_plpc) * 100;
        return <Typography variant="body2" sx={{ color: pct >= 0 ? '#00cc88' : '#ff4444' }}>{formatPercent(pct)}</Typography>;
      },
    },
  ];

  const orderColumns = [
    { key: 'symbol', header: 'Symbol' },
    { key: 'side', header: 'Side', render: (r: BrokerOrder) => <Typography variant="body2" sx={{ color: r.side === 'buy' ? '#00cc88' : '#ff4444' }}>{r.side.toUpperCase()}</Typography> },
    { key: 'qty', header: 'Qty', align: 'right' as const },
    { key: 'type', header: 'Type' },
    { key: 'status', header: 'Status' },
    { key: 'created_at', header: 'Date', render: (r: BrokerOrder) => new Date(r.created_at).toLocaleDateString() },
  ];

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      {account.data && (
        <Grid container spacing={2}>
          <Grid size={{ xs: 6, md: 2.4 }}>
            <MetricCard title="Equity" value={formatCurrency(equity)} />
          </Grid>
          <Grid size={{ xs: 6, md: 2.4 }}>
            <MetricCard title="Cash" value={formatCurrency(safeFloat(account.data.cash))} />
          </Grid>
          <Grid size={{ xs: 6, md: 2.4 }}>
            <MetricCard title="Buying Power" value={formatCurrency(safeFloat(account.data.buying_power))} />
          </Grid>
          <Grid size={{ xs: 6, md: 2.4 }}>
            <MetricCard
              title="Unrealized P&L"
              value={formatCurrency(totalUnrealizedPL)}
              subtitle={formatPercent(totalPLPct)}
              color={totalUnrealizedPL >= 0 ? PORTFOLIO_COLORS.gain : PORTFOLIO_COLORS.loss}
            />
          </Grid>
          <Grid size={{ xs: 6, md: 2.4 }}>
            <MetricCard
              title="Day Change"
              value={formatCurrency(equityChange)}
              subtitle={formatPercent(equityChangePct)}
              color={equityChange >= 0 ? PORTFOLIO_COLORS.gain : PORTFOLIO_COLORS.loss}
            />
          </Grid>
        </Grid>
      )}

      <Grid container spacing={2}>
        <Grid size={{ xs: 12, md: 8 }}>
          <Card>
            <CardContent>
              <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Positions</Typography>
              <DataTable columns={posColumns} data={posData} emptyMessage="No open positions" />
            </CardContent>
          </Card>
        </Grid>
        <Grid size={{ xs: 12, md: 4 }}>
          <Card sx={{ height: '100%' }}>
            <CardContent>
              <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Allocation</Typography>
              {allocationData.length > 0 ? (
                <ResponsiveContainer width="100%" height={250}>
                  <PieChart>
                    <Pie data={allocationData} dataKey="value" nameKey="name" innerRadius={60} outerRadius={90} paddingAngle={2}>
                      {allocationData.map((_, i) => (
                        <Cell key={i} fill={SERIES_PALETTE[i % SERIES_PALETTE.length]} />
                      ))}
                    </Pie>
                    <Tooltip formatter={(val) => formatCurrency(val)} contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }} />
                  </PieChart>
                </ResponsiveContainer>
              ) : (
                <Typography variant="body2" color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>
                  No positions to chart
                </Typography>
              )}
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {plBarData.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>P&L by Position</Typography>
            <ResponsiveContainer width="100%" height={Math.max(200, plBarData.length * 40)}>
              <BarChart data={plBarData} layout="vertical" margin={{ left: 60, right: 20, top: 5, bottom: 5 }}>
                <XAxis type="number" tickFormatter={(v) => formatCurrency(v, 0)} tick={{ fill: '#888', fontSize: 12 }} />
                <YAxis type="category" dataKey="symbol" tick={{ fill: '#ccc', fontSize: 12 }} width={50} />
                <Tooltip
                  formatter={(val) => formatCurrency(val)}
                  contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }}
                />
                <ReferenceLine x={0} stroke="rgba(255,255,255,0.2)" />
                <Bar dataKey="pl" radius={[0, 4, 4, 0]}>
                  {plBarData.map((entry, i) => (
                    <Cell key={i} fill={entry.fill} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardContent>
          <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Recent Orders</Typography>
          <DataTable columns={orderColumns} data={orders.data ?? []} emptyMessage="No orders" />
        </CardContent>
      </Card>
    </Box>
  );
}
