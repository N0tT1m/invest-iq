import { Card, CardContent, Typography, Box, LinearProgress, Alert } from '@mui/material';
import RadarChart from '@/components/RadarChart';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useRiskRadar } from '@/hooks/useRisk';

interface Props {
  symbol: string;
}

const riskColor = (score: number) => {
  if (score >= 75) return '#ff4444';
  if (score >= 50) return '#ffaa00';
  if (score >= 25) return '#00ccff';
  return '#00cc88';
};

export default function RiskRadarPanel({ symbol }: Props) {
  const { data, isLoading, error } = useRiskRadar(symbol);

  if (isLoading) return <LoadingOverlay message="Loading risk radar..." />;
  if (error) return <Alert severity="error">{(error as Error).message}</Alert>;
  if (!data) return null;

  const radarData = data.dimensions.map((d) => ({ name: d.name, value: d.score * 100 }));
  const overallColor = riskColor(data.overall_risk * 100);

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>Risk Radar</Typography>
          <Typography variant="h6" sx={{ color: overallColor, fontWeight: 700 }}>
            {data.risk_level}
          </Typography>
        </Box>

        <RadarChart data={radarData} color={overallColor} />

        <Box sx={{ mt: 2, display: 'flex', flexDirection: 'column', gap: 1 }}>
          {data.dimensions.map((dim) => (
            <Box key={dim.name}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
                <Typography variant="caption" color="text.secondary">{dim.name}</Typography>
                <Typography variant="caption" sx={{ color: riskColor(dim.score * 100) }}>
                  {(dim.score * 100).toFixed(0)}
                </Typography>
              </Box>
              <LinearProgress
                variant="determinate"
                value={dim.score * 100}
                sx={{
                  height: 4,
                  borderRadius: 2,
                  bgcolor: 'rgba(255,255,255,0.1)',
                  '& .MuiLinearProgress-bar': { bgcolor: riskColor(dim.score * 100) },
                }}
              />
            </Box>
          ))}
        </Box>
      </CardContent>
    </Card>
  );
}
