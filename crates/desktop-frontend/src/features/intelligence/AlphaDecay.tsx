import { Card, CardContent, Typography, Box, Grid, Chip, Alert } from '@mui/material';
import LoadingOverlay from '@/components/LoadingOverlay';
import EmptyState from '@/components/EmptyState';
import { useStrategiesHealth } from '@/hooks/useIntelligence';

const statusColor = (status: string) => {
  switch (status.toLowerCase()) {
    case 'healthy': return '#00cc88';
    case 'degrading': case 'warning': return '#ffaa00';
    case 'retired': case 'critical': return '#ff4444';
    default: return '#888888';
  }
};

export default function AlphaDecay() {
  const { data, isLoading, error } = useStrategiesHealth();

  if (isLoading) return <LoadingOverlay message="Loading strategy health..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data || data.length === 0) return <EmptyState title="No strategy data" message="Run a backtest to track alpha decay." />;

  return (
    <Box>
      <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Alpha Decay Monitor</Typography>
      <Grid container spacing={2}>
        {data.map((s) => (
          <Grid key={s.name} size={{ xs: 12, sm: 6, md: 4 }}>
            <Card sx={{ height: '100%' }}>
              <CardContent>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1 }}>
                  <Typography variant="body1" sx={{ fontWeight: 600 }}>{s.name}</Typography>
                  <Chip
                    label={s.status}
                    size="small"
                    sx={{ bgcolor: `${statusColor(s.status)}22`, color: statusColor(s.status), fontWeight: 700 }}
                  />
                </Box>
                <Box sx={{ display: 'flex', gap: 2 }}>
                  <Box>
                    <Typography variant="caption" color="text.secondary">Sharpe</Typography>
                    <Typography variant="body2">{s.sharpe_ratio.toFixed(2)}</Typography>
                  </Box>
                  <Box>
                    <Typography variant="caption" color="text.secondary">Win Rate</Typography>
                    <Typography variant="body2">{(s.win_rate * 100).toFixed(0)}%</Typography>
                  </Box>
                  {s.decay_rate != null && (
                    <Box>
                      <Typography variant="caption" color="text.secondary">Decay</Typography>
                      <Typography variant="body2" sx={{ color: '#ff4444' }}>{(s.decay_rate * 100).toFixed(1)}%</Typography>
                    </Box>
                  )}
                </Box>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
