import { Card, CardContent, Typography, Box, Grid, Alert } from '@mui/material';
import MetricCard from '@/components/MetricCard';
import DataTable from '@/components/DataTable';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useTaxLots, useYearEndSummary } from '@/hooks/useIntelligence';
import { formatCurrency } from '@/utils/format';
import type { TaxLot } from '@/api/types';

export default function TaxDashboard() {
  const lots = useTaxLots();
  const summary = useYearEndSummary();

  if (lots.isLoading || summary.isLoading) return <LoadingOverlay message="Loading tax data..." />;
  if (lots.error) return <Alert severity="error">{(lots.error as Error).message}</Alert>;

  const s = summary.data;

  const lotColumns = [
    { key: 'symbol', header: 'Symbol', render: (r: TaxLot) => <Typography variant="body2" fontWeight={600}>{r.symbol}</Typography> },
    { key: 'qty', header: 'Qty', align: 'right' as const },
    { key: 'cost_basis', header: 'Cost Basis', align: 'right' as const, render: (r: TaxLot) => formatCurrency(r.cost_basis) },
    { key: 'acquired_date', header: 'Acquired', render: (r: TaxLot) => r.acquired_date.slice(0, 10) },
    { key: 'holding_period', header: 'Period' },
    {
      key: 'unrealized_pnl', header: 'Unrealized', align: 'right' as const,
      render: (r: TaxLot) => r.unrealized_pnl != null ? (
        <Typography variant="body2" sx={{ color: r.unrealized_pnl >= 0 ? '#00cc88' : '#ff4444' }}>
          {formatCurrency(r.unrealized_pnl)}
        </Typography>
      ) : '-',
    },
  ];

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      {s && (
        <Grid container spacing={2}>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Short-Term" value={formatCurrency(s.short_term_gains)} color={s.short_term_gains >= 0 ? '#00cc88' : '#ff4444'} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Long-Term" value={formatCurrency(s.long_term_gains)} color={s.long_term_gains >= 0 ? '#00cc88' : '#ff4444'} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Total Gains" value={formatCurrency(s.total_gains)} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Wash Sales" value={formatCurrency(s.wash_sale_adjustments)} color="#ffaa00" />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Harvested" value={formatCurrency(s.harvested_losses)} color="#ff4444" />
          </Grid>
        </Grid>
      )}

      <Card>
        <CardContent>
          <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Tax Lots</Typography>
          <DataTable columns={lotColumns} data={lots.data ?? []} emptyMessage="No tax lots" />
        </CardContent>
      </Card>
    </Box>
  );
}
