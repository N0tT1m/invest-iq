import { Card, CardContent, Typography, Box, Chip, Grid, Alert } from '@mui/material';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMacroIndicators } from '@/hooks/useIntelligence';

const regimeColor = (regime: string) => {
  switch (regime.toLowerCase()) {
    case 'risk_on': case 'risk on': return '#00cc88';
    case 'risk_off': case 'risk off': return '#ff4444';
    default: return '#ffaa00';
  }
};

export default function MacroOverlay() {
  const { data, isLoading, error } = useMacroIndicators();

  if (isLoading) return <LoadingOverlay message="Loading macro data..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Macro Overlay</Typography>
          <Chip
            label={data.regime}
            size="small"
            sx={{ bgcolor: `${regimeColor(data.regime)}22`, color: regimeColor(data.regime), fontWeight: 700 }}
          />
        </Box>
        <Grid container spacing={2}>
          <Grid size={{ xs: 6 }}>
            <Box>
              <Typography variant="caption" color="text.secondary">Trend</Typography>
              <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.trend}</Typography>
            </Box>
          </Grid>
          <Grid size={{ xs: 6 }}>
            <Box>
              <Typography variant="caption" color="text.secondary">Rates</Typography>
              <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.rates_direction}</Typography>
            </Box>
          </Grid>
          <Grid size={{ xs: 6 }}>
            <Box>
              <Typography variant="caption" color="text.secondary">Volatility</Typography>
              <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.volatility}</Typography>
            </Box>
          </Grid>
        </Grid>
      </CardContent>
    </Card>
  );
}
