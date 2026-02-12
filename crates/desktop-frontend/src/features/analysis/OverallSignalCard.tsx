import { Card, CardContent, Box, Typography } from '@mui/material';
import StatusBadge from '@/components/StatusBadge';
import GaugeChart from '@/components/GaugeChart';
import { formatCurrency, signalColor } from '@/utils/format';
import type { AnalysisResult } from '@/api/types';

interface Props {
  analysis: AnalysisResult;
}

export default function OverallSignalCard({ analysis }: Props) {
  const color = signalColor(analysis.overall_signal);

  return (
    <Card>
      <CardContent>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <Box>
            <Typography variant="h5" sx={{ fontWeight: 700 }}>
              {analysis.symbol}
            </Typography>
            {analysis.current_price != null && (
              <Typography variant="h6" color="text.secondary">
                {formatCurrency(analysis.current_price)}
              </Typography>
            )}
            <Box sx={{ mt: 1 }}>
              <StatusBadge signal={analysis.overall_signal} size="medium" />
            </Box>
          </Box>
          <GaugeChart
            value={analysis.overall_confidence * 100}
            label="Confidence"
            color={color}
            size={140}
          />
        </Box>
      </CardContent>
    </Card>
  );
}
