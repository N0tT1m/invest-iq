import { Card, CardContent, Typography, Box, LinearProgress, Alert } from '@mui/material';
import GaugeChart from '@/components/GaugeChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useSocialSentimentIntel } from '@/hooks/useIntelligence';
import { SENTIMENT_COLORS } from '@/theme/colors';

interface Props { symbol: string }

export default function SocialSentiment({ symbol }: Props) {
  const { data, isLoading, error } = useSocialSentimentIntel(symbol);

  if (isLoading) return <LoadingOverlay message="Loading social sentiment..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const gaugeVal = (data.overall_score + 1) * 50; // -1..1 → 0..100
  const color = data.sentiment_label === 'positive' ? SENTIMENT_COLORS.positive
    : data.sentiment_label === 'negative' ? SENTIMENT_COLORS.negative
    : SENTIMENT_COLORS.neutral;

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Social Sentiment</Typography>
        <Box sx={{ display: 'flex', justifyContent: 'center', mb: 2 }}>
          <GaugeChart value={gaugeVal} label={data.sentiment_label} color={color} size={140} />
        </Box>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          {Object.entries(data.components).map(([name, val]) => (
            <Box key={name}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
                <Typography variant="caption" color="text.secondary">{name.replace(/_/g, ' ')}</Typography>
                <Typography variant="caption">{(val * 100).toFixed(0)}</Typography>
              </Box>
              <LinearProgress
                variant="determinate"
                value={Math.abs(val) * 100}
                sx={{ height: 4, borderRadius: 2, bgcolor: 'rgba(255,255,255,0.1)', '& .MuiLinearProgress-bar': { bgcolor: val >= 0 ? SENTIMENT_COLORS.positive : SENTIMENT_COLORS.negative } }}
              />
            </Box>
          ))}
        </Box>
      </CardContent>
    </Card>
  );
}
