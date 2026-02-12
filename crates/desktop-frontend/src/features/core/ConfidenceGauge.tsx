import { Card, CardContent, Typography, Box, LinearProgress } from '@mui/material';
import GaugeChart from '@/components/GaugeChart';
import type { AnalysisResult } from '@/api/types';

interface Props {
  analysis: AnalysisResult;
}

export default function ConfidenceGauge({ analysis }: Props) {
  const engines = [
    { name: 'Technical', confidence: analysis.technical.confidence },
    { name: 'Fundamental', confidence: analysis.fundamental.confidence },
    { name: 'Quantitative', confidence: analysis.quantitative.confidence },
    { name: 'Sentiment', confidence: analysis.sentiment.confidence },
  ];

  const overall = analysis.overall_confidence * 100;
  const color = overall >= 70 ? '#00cc88' : overall >= 40 ? '#ffaa00' : '#ff4444';

  return (
    <Card>
      <CardContent>
        <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>
          Confidence Breakdown
        </Typography>

        <Box sx={{ display: 'flex', justifyContent: 'center', mb: 3 }}>
          <GaugeChart value={overall} label="Overall" color={color} size={160} />
        </Box>

        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          {engines.map((eng) => {
            const val = eng.confidence * 100;
            return (
              <Box key={eng.name}>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
                  <Typography variant="caption" color="text.secondary">{eng.name}</Typography>
                  <Typography variant="caption">{val.toFixed(0)}%</Typography>
                </Box>
                <LinearProgress
                  variant="determinate"
                  value={val}
                  sx={{
                    height: 5,
                    borderRadius: 2.5,
                    bgcolor: 'rgba(255,255,255,0.1)',
                    '& .MuiLinearProgress-bar': { borderRadius: 2.5 },
                  }}
                />
              </Box>
            );
          })}
        </Box>
      </CardContent>
    </Card>
  );
}
