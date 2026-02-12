import { Card, CardContent, Typography, Box, Alert } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, Legend } from 'recharts';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useMLCalibration } from '@/hooks/useML';

interface Props { symbol: string }

export default function MLCalibration({ symbol }: Props) {
  const { data, isLoading, error } = useMLCalibration(symbol);

  if (isLoading) return <LoadingOverlay message="Loading calibration..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const barData = [
    { name: 'Confidence', raw: data.raw * 100, calibrated: data.calibrated * 100 },
  ];

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>ML Calibration</Typography>

        <ResponsiveContainer width="100%" height={120}>
          <BarChart data={barData} layout="vertical">
            <XAxis type="number" domain={[0, 100]} tick={{ fill: '#a0a0a0', fontSize: 11 }} />
            <YAxis type="category" dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 11 }} width={80} />
            <Tooltip contentStyle={{ background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 }} />
            <Legend wrapperStyle={{ fontSize: 11 }} />
            <Bar dataKey="raw" fill="#667eea" name="Raw" radius={[0, 4, 4, 0]} />
            <Bar dataKey="calibrated" fill="#38ef7d" name="Calibrated" radius={[0, 4, 4, 0]} />
          </BarChart>
        </ResponsiveContainer>

        {data.components && Object.keys(data.components).length > 0 && (
          <Box sx={{ mt: 2 }}>
            <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>Uncertainty Components</Typography>
            {Object.entries(data.components).map(([name, val]) => (
              <Box key={name} sx={{ display: 'flex', justifyContent: 'space-between', py: 0.5 }}>
                <Typography variant="caption" color="text.secondary">{name.replace(/_/g, ' ')}</Typography>
                <Typography variant="caption">{(val * 100).toFixed(1)}%</Typography>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
