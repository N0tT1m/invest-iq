import { Card, CardContent, Typography, Box, Grid, Chip, Alert } from '@mui/material';
import StatusBadge from '@/components/StatusBadge';
import LoadingOverlay from '@/components/LoadingOverlay';
import EmptyState from '@/components/EmptyState';
import { useOpportunities } from '@/hooks/useIntelligence';
import { formatCurrency, formatPercent } from '@/utils/format';

export default function SmartWatchlist() {
  const { data, isLoading, error } = useOpportunities();

  if (isLoading) return <LoadingOverlay message="Scanning opportunities..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data || data.length === 0) return <EmptyState title="No opportunities" message="No opportunities found at this time." />;

  return (
    <Box>
      <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>
        Smart Watchlist ({data.length} opportunities)
      </Typography>
      <Grid container spacing={2}>
        {data.map((item) => (
          <Grid key={item.symbol} size={{ xs: 12, sm: 6, md: 4 }}>
            <Card sx={{ height: '100%' }}>
              <CardContent>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1 }}>
                  <Typography variant="h6" sx={{ fontWeight: 700 }}>{item.symbol}</Typography>
                  {item.signal && <StatusBadge signal={item.signal} />}
                </Box>
                <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                  {item.price != null && <Chip label={formatCurrency(item.price)} size="small" variant="outlined" />}
                  {item.change_pct != null && (
                    <Chip
                      label={formatPercent(item.change_pct)}
                      size="small"
                      sx={{ color: item.change_pct >= 0 ? '#00cc88' : '#ff4444' }}
                    />
                  )}
                  {item.score != null && <Chip label={`Score: ${item.score.toFixed(0)}`} size="small" variant="outlined" />}
                </Box>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
