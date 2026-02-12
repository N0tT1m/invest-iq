import { Card, CardContent, Typography, Alert } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, Cell } from 'recharts';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useSectorFlows } from '@/hooks/useIntelligence';

export default function FlowMap() {
  const { data, isLoading, error } = useSectorFlows();

  if (isLoading) return <LoadingOverlay message="Loading sector flows..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data || data.length === 0) return null;

  const chartData = data
    .sort((a, b) => b.change_pct - a.change_pct)
    .map((s) => ({ name: s.etf, value: s.change_pct, sector: s.sector }));

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Sector Flow Map</Typography>
        <ResponsiveContainer width="100%" height={300}>
          <BarChart data={chartData}>
            <XAxis dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 10 }} />
            <YAxis tick={{ fill: '#a0a0a0', fontSize: 11 }} tickFormatter={(v) => `${v}%`} />
            <Tooltip
              contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }}
              formatter={(v) => `${Number(v).toFixed(2)}%`}
              labelFormatter={(label) => {
                const item = chartData.find((d) => d.name === label);
                return item ? `${item.sector} (${item.name})` : String(label);
              }}
            />
            <Bar dataKey="value" radius={[4, 4, 0, 0]}>
              {chartData.map((entry, i) => (
                <Cell key={i} fill={entry.value >= 0 ? '#38ef7d' : '#f45c43'} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
