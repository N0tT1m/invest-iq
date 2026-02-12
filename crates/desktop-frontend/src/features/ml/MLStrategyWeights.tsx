import { Card, CardContent, Typography, Alert } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, ErrorBar } from 'recharts';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMLStrategyWeights } from '@/hooks/useML';

export default function MLStrategyWeights() {
  const { data, isLoading, error } = useMLStrategyWeights();

  if (isLoading) return <LoadingOverlay message="Loading strategy weights..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const chartData = data.weights.map((w) => ({
    name: w.engine,
    weight: w.weight * 100,
    error: w.credible_high != null && w.credible_low != null
      ? [(w.weight - w.credible_low) * 100, (w.credible_high - w.weight) * 100]
      : undefined,
  }));

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>ML Strategy Weights</Typography>
        <ResponsiveContainer width="100%" height={250}>
          <BarChart data={chartData}>
            <XAxis dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 11 }} />
            <YAxis tick={{ fill: '#a0a0a0', fontSize: 11 }} domain={[0, 100]} tickFormatter={(v) => `${v}%`} />
            <Tooltip
              contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }}
              formatter={(v) => `${Number(v).toFixed(1)}%`}
            />
            <Bar dataKey="weight" fill="#667eea" radius={[4, 4, 0, 0]}>
              <ErrorBar dataKey="error" width={4} stroke="#a0a0a0" />
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
