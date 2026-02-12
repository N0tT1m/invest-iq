import { Card, CardContent, Typography, Box, LinearProgress } from '@mui/material';
import StatusBadge from '@/components/StatusBadge';
import { signalColor } from '@/utils/format';
import type { EngineResult } from '@/api/types';

interface Props {
  title: string;
  engine: EngineResult;
}

export default function EngineCard({ title, engine }: Props) {
  const color = signalColor(engine.signal);
  const confidence = engine.confidence * 100;

  // Extract notable details for display
  const detailEntries = Object.entries(engine.details).slice(0, 6);

  return (
    <Card sx={{ height: '100%' }}>
      <CardContent>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>{title}</Typography>
          <StatusBadge signal={engine.signal} />
        </Box>

        <Box sx={{ mb: 2 }}>
          <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 0.5 }}>
            <Typography variant="caption" color="text.secondary">Confidence</Typography>
            <Typography variant="caption" sx={{ color }}>{confidence.toFixed(0)}%</Typography>
          </Box>
          <LinearProgress
            variant="determinate"
            value={confidence}
            sx={{
              height: 6,
              borderRadius: 3,
              bgcolor: 'rgba(255,255,255,0.1)',
              '& .MuiLinearProgress-bar': { bgcolor: color, borderRadius: 3 },
            }}
          />
        </Box>

        {detailEntries.length > 0 && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
            {detailEntries.map(([key, val]) => (
              <Box key={key} sx={{ display: 'flex', justifyContent: 'space-between' }}>
                <Typography variant="caption" color="text.secondary">
                  {key.replace(/_/g, ' ')}
                </Typography>
                <Typography variant="caption">
                  {typeof val === 'number' ? val.toFixed(2) : String(val)}
                </Typography>
              </Box>
            ))}
          </Box>
        )}
      </CardContent>
    </Card>
  );
}
