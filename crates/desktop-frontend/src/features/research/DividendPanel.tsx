import { Card, CardContent, Typography, Box, Chip, Alert } from '@mui/material';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useDividends } from '@/hooks/useResearch';
import { formatPercent } from '@/utils/format';

interface Props { symbol: string }

export default function DividendPanel({ symbol }: Props) {
  const { data, isLoading, error } = useDividends(symbol);

  if (isLoading) return <LoadingOverlay message="Loading dividends..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Dividends</Typography>
          {data.data_source && <Chip label={data.data_source} size="small" variant="outlined" />}
        </Box>
        <Box sx={{ display: 'flex', gap: 3, mb: 2, flexWrap: 'wrap' }}>
          {data.yield_pct != null && (
            <Box>
              <Typography variant="caption" color="text.secondary">Yield</Typography>
              <Typography variant="h6" sx={{ color: '#00cc88' }}>{formatPercent(data.yield_pct)}</Typography>
            </Box>
          )}
          {data.frequency && (
            <Box>
              <Typography variant="caption" color="text.secondary">Frequency</Typography>
              <Typography variant="h6">{data.frequency}</Typography>
            </Box>
          )}
          {data.growth_rate != null && (
            <Box>
              <Typography variant="caption" color="text.secondary">Growth Rate</Typography>
              <Typography variant="h6" sx={{ color: data.growth_rate >= 0 ? '#00cc88' : '#ff4444' }}>{formatPercent(data.growth_rate)}</Typography>
            </Box>
          )}
        </Box>
        {data.history.length > 0 && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>History</Typography>
            {data.history.slice(0, 4).map((h, i) => (
              <Box key={i} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5, borderBottom: '1px solid', borderColor: 'divider' }}>
                <Typography variant="caption">{String(h.date ?? h.ex_date ?? `#${i + 1}`)}</Typography>
                <Typography variant="caption">{String(h.amount ?? h.value ?? '')}</Typography>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
