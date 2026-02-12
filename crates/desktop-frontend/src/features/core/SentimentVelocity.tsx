import { Card, CardContent, Typography, Box, Alert } from '@mui/material';
import { LineChart, Line, ResponsiveContainer, XAxis, YAxis, Tooltip } from 'recharts';
import GaugeChart from '@/components/GaugeChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useSentimentVelocity } from '@/hooks/useSentiment';

interface Props {
  symbol: string;
}

export default function SentimentVelocityPanel({ symbol }: Props) {
  const { data, isLoading, error } = useSentimentVelocity(symbol);

  if (isLoading) return <LoadingOverlay message="Loading sentiment..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  // Normalize velocity to 0-100 gauge range (velocity is typically -1 to 1)
  const gaugeVal = Math.max(0, Math.min(100, (data.velocity + 1) * 50));
  const color = data.velocity > 0 ? '#00cc88' : data.velocity < -0.3 ? '#ff4444' : '#ffaa00';

  const history = data.history?.map((h) => ({
    time: new Date(h.timestamp).toLocaleDateString(),
    score: h.score,
  })) ?? [];

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Sentiment Velocity</Typography>
          <Typography variant="caption" sx={{ color }}>
            {data.trend}
          </Typography>
        </Box>

        <Box sx={{ display: 'flex', justifyContent: 'center', mb: 2 }}>
          <GaugeChart value={gaugeVal} label="Velocity" color={color} size={140} />
        </Box>

        <Box sx={{ display: 'flex', gap: 3, justifyContent: 'center', mb: 2 }}>
          <Box sx={{ textAlign: 'center' }}>
            <Typography variant="caption" color="text.secondary">Velocity</Typography>
            <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.velocity.toFixed(3)}</Typography>
          </Box>
          <Box sx={{ textAlign: 'center' }}>
            <Typography variant="caption" color="text.secondary">Acceleration</Typography>
            <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.acceleration.toFixed(3)}</Typography>
          </Box>
          <Box sx={{ textAlign: 'center' }}>
            <Typography variant="caption" color="text.secondary">Current</Typography>
            <Typography variant="body2" sx={{ fontWeight: 600 }}>{data.current_sentiment.toFixed(2)}</Typography>
          </Box>
        </Box>

        {history.length > 0 && (
          <ResponsiveContainer width="100%" height={100}>
            <LineChart data={history}>
              <XAxis dataKey="time" hide />
              <YAxis domain={[-1, 1]} hide />
              <Tooltip
                contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }}
                labelStyle={{ color: '#a0a0a0' }}
              />
              <Line type="monotone" dataKey="score" stroke={color} strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        )}
      </CardContent>
    </Card>
  );
}
