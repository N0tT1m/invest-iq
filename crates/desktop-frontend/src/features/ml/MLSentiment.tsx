import { Card, CardContent, Typography, Box, Alert } from '@mui/material';
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from 'recharts';
import GaugeChart from '@/components/GaugeChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMLSentiment } from '@/hooks/useML';
import { SENTIMENT_COLORS } from '@/theme/colors';

interface Props { symbol: string }

export default function MLSentiment({ symbol }: Props) {
  const { data, isLoading, error } = useMLSentiment(symbol);

  if (isLoading) return <LoadingOverlay message="Loading ML sentiment..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const gaugeVal = (data.score + 1) * 50;
  const color = data.label === 'positive' ? SENTIMENT_COLORS.positive
    : data.label === 'negative' ? SENTIMENT_COLORS.negative
    : SENTIMENT_COLORS.neutral;

  const distData = [
    { name: 'Positive', value: data.distribution.positive, color: SENTIMENT_COLORS.positive },
    { name: 'Neutral', value: data.distribution.neutral, color: SENTIMENT_COLORS.neutral },
    { name: 'Negative', value: data.distribution.negative, color: SENTIMENT_COLORS.negative },
  ];

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>ML Sentiment (FinBERT)</Typography>

        <Box sx={{ display: 'flex', justifyContent: 'space-around', alignItems: 'center' }}>
          <GaugeChart value={gaugeVal} label={data.label} color={color} size={130} />

          <ResponsiveContainer width="50%" height={150}>
            <PieChart>
              <Pie data={distData} dataKey="value" innerRadius={35} outerRadius={55} paddingAngle={3}>
                {distData.map((d, i) => <Cell key={i} fill={d.color} />)}
              </Pie>
              <Tooltip
                contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }}
                formatter={(v) => `${(Number(v) * 100).toFixed(1)}%`}
              />
            </PieChart>
          </ResponsiveContainer>
        </Box>
      </CardContent>
    </Card>
  );
}
