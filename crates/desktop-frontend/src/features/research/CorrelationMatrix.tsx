import { Card, CardContent, Typography, Box, Alert } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, Cell } from 'recharts';
import { LineChart, Line } from 'recharts';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useCorrelations } from '@/hooks/useResearch';

interface Props { symbol: string }

export default function CorrelationMatrix({ symbol }: Props) {
  const { data, isLoading, error } = useCorrelations(symbol);

  if (isLoading) return <LoadingOverlay message="Loading correlations..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const benchmarkData = Object.entries(data.benchmarks).map(([name, value]) => ({
    name,
    value: Number(value.toFixed(2)),
  }));

  const rollingData = data.rolling_correlation?.map((d) => ({
    date: new Date(d.date).toLocaleDateString(),
    value: d.value,
  })) ?? [];

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Correlation: {symbol}</Typography>

        <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>Benchmark Correlations</Typography>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={benchmarkData} layout="vertical">
            <XAxis type="number" domain={[-1, 1]} tick={{ fill: '#a0a0a0', fontSize: 11 }} />
            <YAxis type="category" dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 11 }} width={50} />
            <Tooltip contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }} />
            <Bar dataKey="value" radius={[0, 4, 4, 0]}>
              {benchmarkData.map((entry, i) => (
                <Cell key={i} fill={entry.value >= 0 ? '#667eea' : '#f5576c'} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>

        {rollingData.length > 0 && (
          <Box sx={{ mt: 2 }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>30d Rolling Correlation</Typography>
            <ResponsiveContainer width="100%" height={120}>
              <LineChart data={rollingData}>
                <XAxis dataKey="date" hide />
                <YAxis domain={[-1, 1]} hide />
                <Tooltip contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }} />
                <Line type="monotone" dataKey="value" stroke="#667eea" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
