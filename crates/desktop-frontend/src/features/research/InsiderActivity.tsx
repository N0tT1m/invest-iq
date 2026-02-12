import { Card, CardContent, Typography, Box, Chip, Alert } from '@mui/material';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useInsiders } from '@/hooks/useResearch';

interface Props { symbol: string }

export default function InsiderActivity({ symbol }: Props) {
  const { data, isLoading, error } = useInsiders(symbol);

  if (isLoading) return <LoadingOverlay message="Loading insider data..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const sentimentColor = data.net_sentiment === 'bullish' ? '#00cc88' : data.net_sentiment === 'bearish' ? '#ff4444' : '#888888';

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Insider Activity</Typography>
          <Box sx={{ display: 'flex', gap: 1 }}>
            {data.net_sentiment && <Chip label={data.net_sentiment} size="small" sx={{ bgcolor: `${sentimentColor}22`, color: sentimentColor }} />}
            {data.data_source && <Chip label={data.data_source} size="small" variant="outlined" />}
          </Box>
        </Box>

        {data.transactions.length === 0 ? (
          <Typography variant="body2" color="text.secondary" sx={{ textAlign: 'center', py: 2 }}>
            No recent insider transactions
          </Typography>
        ) : (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            {data.transactions.slice(0, 8).map((tx, i) => (
              <Box key={i} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5, borderBottom: '1px solid', borderColor: 'divider' }}>
                <Box>
                  <Typography variant="caption" sx={{ fontWeight: 600 }}>{String(tx.insider ?? tx.name ?? 'Insider')}</Typography>
                  <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>{String(tx.type ?? tx.transaction_type ?? '')}</Typography>
                </Box>
                <Box sx={{ textAlign: 'right' }}>
                  <Typography variant="caption">{String(tx.shares ?? tx.amount ?? '')}</Typography>
                  {tx.date ? <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>{String(tx.date)}</Typography> : null}
                </Box>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
