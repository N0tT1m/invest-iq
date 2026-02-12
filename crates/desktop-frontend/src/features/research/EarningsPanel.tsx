import { Card, CardContent, Typography, Box, Chip, Alert } from '@mui/material';
import MetricCard from '@/components/MetricCard';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useEarnings } from '@/hooks/useResearch';
import { formatPercent } from '@/utils/format';

interface Props { symbol: string }

export default function EarningsPanel({ symbol }: Props) {
  const { data, isLoading, error } = useEarnings(symbol);

  if (isLoading) return <LoadingOverlay message="Loading earnings..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Earnings</Typography>
          {data.data_source && <Chip label={data.data_source} size="small" variant="outlined" />}
        </Box>
        <Box sx={{ display: 'flex', gap: 2, mb: 2 }}>
          {data.eps_growth != null && <MetricCard title="EPS Growth" value={formatPercent(data.eps_growth)} color={data.eps_growth >= 0 ? '#00cc88' : '#ff4444'} />}
          {data.revenue_growth != null && <MetricCard title="Revenue Growth" value={formatPercent(data.revenue_growth)} color={data.revenue_growth >= 0 ? '#00cc88' : '#ff4444'} />}
        </Box>
        {data.history.length > 0 && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>Recent History</Typography>
            {data.history.slice(0, 4).map((h, i) => (
              <Box key={i} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5, borderBottom: '1px solid', borderColor: 'divider' }}>
                <Typography variant="caption">{String(h.period ?? h.date ?? `Q${i + 1}`)}</Typography>
                <Typography variant="caption">{String(h.eps ?? h.value ?? '')}</Typography>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
