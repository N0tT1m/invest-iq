import { Card, CardContent, Typography, Box, LinearProgress, Alert } from '@mui/material';
import GaugeChart from '@/components/GaugeChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useShortInterest } from '@/hooks/useResearch';

interface Props { symbol: string }

export default function ShortInterest({ symbol }: Props) {
  const { data, isLoading, error } = useShortInterest(symbol);

  if (isLoading) return <LoadingOverlay message="Loading short interest..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const score = data.short_interest_score * 100;
  const color = score >= 70 ? '#ff4444' : score >= 40 ? '#ffaa00' : '#00cc88';

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Short Interest</Typography>

        <Box sx={{ display: 'flex', justifyContent: 'center', mb: 2 }}>
          <GaugeChart value={score} label="Squeeze Risk" color={color} size={140} />
        </Box>

        {data.interpretation && (
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2, textAlign: 'center' }}>
            {data.interpretation}
          </Typography>
        )}

        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          {Object.entries(data.components).map(([name, val]) => (
            <Box key={name}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
                <Typography variant="caption" color="text.secondary">{name.replace(/_/g, ' ')}</Typography>
                <Typography variant="caption">{(val * 100).toFixed(0)}</Typography>
              </Box>
              <LinearProgress
                variant="determinate"
                value={val * 100}
                sx={{ height: 4, borderRadius: 2, bgcolor: 'rgba(255,255,255,0.1)', '& .MuiLinearProgress-bar': { borderRadius: 2 } }}
              />
            </Box>
          ))}
        </Box>
      </CardContent>
    </Card>
  );
}
