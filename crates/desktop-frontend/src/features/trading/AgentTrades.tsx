import { Box, Card, CardContent, Typography, Button, Chip, Alert, Grid } from '@mui/material';
import { ThumbUp, ThumbDown } from '@mui/icons-material';
import LoadingOverlay from '@/components/LoadingOverlay';
import EmptyState from '@/components/EmptyState';
import StatusBadge from '@/components/StatusBadge';
import { usePendingTrades, useReviewTrade } from '@/hooks/useAgent';
import { formatPercent } from '@/utils/format';

export default function AgentTrades() {
  const { data: trades, isLoading, error } = usePendingTrades();
  const review = useReviewTrade();

  if (isLoading) return <LoadingOverlay message="Loading agent trades..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!trades || trades.length === 0) return <EmptyState title="No pending trades" message="The agent hasn't proposed any trades yet." />;

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
        Pending Agent Trades ({trades.length})
      </Typography>

      <Grid container spacing={2}>
        {trades.map((trade) => (
          <Grid key={trade.id} size={{ xs: 12, md: 6 }}>
            <Card>
              <CardContent>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1 }}>
                  <Typography variant="h6" sx={{ fontWeight: 700 }}>{trade.symbol}</Typography>
                  <StatusBadge signal={trade.signal} />
                </Box>

                <Box sx={{ display: 'flex', gap: 1, mb: 1, flexWrap: 'wrap' }}>
                  <Chip label={trade.side.toUpperCase()} size="small" color={trade.side === 'buy' ? 'success' : 'error'} variant="outlined" />
                  <Chip label={`${trade.qty} shares`} size="small" variant="outlined" />
                  <Chip label={`${formatPercent(trade.confidence * 100)} conf`} size="small" variant="outlined" />
                  {trade.regime && <Chip label={trade.regime} size="small" variant="outlined" />}
                </Box>

                {trade.reasoning && (
                  <Typography variant="body2" color="text.secondary" sx={{ mb: 2, fontSize: 12 }}>
                    {trade.reasoning}
                  </Typography>
                )}

                <Box sx={{ display: 'flex', gap: 1 }}>
                  <Button
                    size="small"
                    variant="contained"
                    color="success"
                    startIcon={<ThumbUp />}
                    onClick={() => review.mutate({ id: trade.id, action: 'approve' })}
                    disabled={review.isPending}
                  >
                    Approve
                  </Button>
                  <Button
                    size="small"
                    variant="outlined"
                    color="error"
                    startIcon={<ThumbDown />}
                    onClick={() => review.mutate({ id: trade.id, action: 'reject' })}
                    disabled={review.isPending}
                  >
                    Reject
                  </Button>
                </Box>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
