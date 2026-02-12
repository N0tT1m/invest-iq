import { Card, CardContent, Typography, Box, LinearProgress, Alert } from '@mui/material';
import GaugeChart from '@/components/GaugeChart';
import StatusBadge from '@/components/StatusBadge';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMLTradeSignal } from '@/hooks/useML';

interface Props { symbol: string }

export default function MLTradeSignal({ symbol }: Props) {
  const { data, isLoading, error } = useMLTradeSignal(symbol);

  if (isLoading) return <LoadingOverlay message="Loading ML signal..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const probPct = data.probability * 100;
  const color = probPct >= 70 ? '#00cc88' : probPct >= 40 ? '#ffaa00' : '#ff4444';

  const topFeatures = Object.entries(data.features)
    .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))
    .slice(0, 8);

  const maxImportance = Math.max(...topFeatures.map(([, v]) => Math.abs(v)), 0.01);

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>ML Trade Signal</Typography>
          <StatusBadge signal={data.signal} />
        </Box>

        <Box sx={{ display: 'flex', justifyContent: 'center', mb: 2 }}>
          <GaugeChart value={probPct} label="Probability" color={color} size={140} />
        </Box>

        <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600, mb: 1 }}>Feature Importance</Typography>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
          {topFeatures.map(([name, val]) => (
            <Box key={name}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
                <Typography variant="caption" color="text.secondary">{name.replace(/_/g, ' ')}</Typography>
                <Typography variant="caption">{val.toFixed(3)}</Typography>
              </Box>
              <LinearProgress
                variant="determinate"
                value={(Math.abs(val) / maxImportance) * 100}
                sx={{ height: 4, borderRadius: 2, bgcolor: 'rgba(255,255,255,0.1)', '& .MuiLinearProgress-bar': { bgcolor: val >= 0 ? '#00cc88' : '#ff4444' } }}
              />
            </Box>
          ))}
        </Box>
      </CardContent>
    </Card>
  );
}
