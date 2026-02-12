import { Card, CardContent, Typography, Box, Chip, Alert } from '@mui/material';
import GaugeChart from '@/components/GaugeChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useOptions } from '@/hooks/useResearch';

interface Props { symbol: string }

export default function OptionsFlow({ symbol }: Props) {
  const { data, isLoading, error } = useOptions(symbol);

  if (isLoading) return <LoadingOverlay message="Loading options..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const pcr = data.put_call_ratio ?? 0;
  const pcrGauge = Math.min(100, pcr * 50); // Normalize: 2.0 = 100
  const pcrColor = pcr > 1.2 ? '#ff4444' : pcr < 0.8 ? '#00cc88' : '#ffaa00';

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Options Flow</Typography>
          {data.data_source && <Chip label={data.data_source} size="small" variant="outlined" />}
        </Box>

        <Box sx={{ display: 'flex', gap: 4, alignItems: 'center', mb: 2, justifyContent: 'center' }}>
          <GaugeChart value={pcrGauge} label="Put/Call" color={pcrColor} size={120} />
          {data.implied_volatility != null && (
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant="caption" color="text.secondary">IV</Typography>
              <Typography variant="h6">{(data.implied_volatility * 100).toFixed(1)}%</Typography>
            </Box>
          )}
        </Box>

        {data.flows.length > 0 && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>Flow Activity</Typography>
            {data.flows.slice(0, 5).map((f, i) => (
              <Box key={i} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5, borderBottom: '1px solid', borderColor: 'divider' }}>
                <Typography variant="caption">{String(f.type ?? f.side ?? 'trade')}</Typography>
                <Typography variant="caption">{String(f.value ?? f.premium ?? '')}</Typography>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
