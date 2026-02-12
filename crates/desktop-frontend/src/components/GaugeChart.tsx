import { Box, Typography } from '@mui/material';

interface GaugeChartProps {
  value: number; // 0–100
  label?: string;
  size?: number;
  color?: string;
  trackColor?: string;
}

export default function GaugeChart({
  value,
  label,
  size = 120,
  color = '#667eea',
  trackColor = 'rgba(255,255,255,0.1)',
}: GaugeChartProps) {
  const clampedValue = Math.max(0, Math.min(100, value));
  const radius = (size - 12) / 2;
  const cx = size / 2;
  const cy = size / 2;

  // Arc from -135deg to +135deg (270deg sweep)
  const startAngle = -225;
  const sweepAngle = 270;
  const endAngle = startAngle + sweepAngle;
  const filledAngle = startAngle + (sweepAngle * clampedValue) / 100;

  const trackPath = describeArc(cx, cy, radius, startAngle, endAngle);
  const fillPath = clampedValue > 0 ? describeArc(cx, cy, radius, startAngle, filledAngle) : '';

  return (
    <Box sx={{ display: 'inline-flex', flexDirection: 'column', alignItems: 'center' }}>
      <svg width={size} height={size * 0.75} viewBox={`0 0 ${size} ${size * 0.85}`}>
        <path d={trackPath} fill="none" stroke={trackColor} strokeWidth={8} strokeLinecap="round" />
        {fillPath && (
          <path d={fillPath} fill="none" stroke={color} strokeWidth={8} strokeLinecap="round" />
        )}
        <text
          x={cx}
          y={cy - 2}
          textAnchor="middle"
          dominantBaseline="middle"
          fill="white"
          fontSize={size * 0.22}
          fontWeight={700}
        >
          {Math.round(clampedValue)}
        </text>
      </svg>
      {label && (
        <Typography variant="caption" color="text.secondary" sx={{ mt: -1 }}>
          {label}
        </Typography>
      )}
    </Box>
  );
}

function describeArc(cx: number, cy: number, r: number, startAngle: number, endAngle: number): string {
  const toRad = (deg: number) => (deg * Math.PI) / 180;
  const start = { x: cx + r * Math.cos(toRad(startAngle)), y: cy + r * Math.sin(toRad(startAngle)) };
  const end = { x: cx + r * Math.cos(toRad(endAngle)), y: cy + r * Math.sin(toRad(endAngle)) };
  const largeArc = Math.abs(endAngle - startAngle) > 180 ? 1 : 0;
  return `M ${start.x} ${start.y} A ${r} ${r} 0 ${largeArc} 1 ${end.x} ${end.y}`;
}
