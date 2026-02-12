import {
  Radar,
  RadarChart as RechartsRadar,
  PolarGrid,
  PolarAngleAxis,
  PolarRadiusAxis,
  ResponsiveContainer,
} from 'recharts';

interface RadarChartProps {
  data: { name: string; value: number }[];
  color?: string;
  fillOpacity?: number;
  size?: number;
}

export default function RadarChart({
  data,
  color = '#667eea',
  fillOpacity = 0.3,
}: RadarChartProps) {
  return (
    <ResponsiveContainer width="100%" height={250}>
      <RechartsRadar data={data}>
        <PolarGrid stroke="rgba(255,255,255,0.1)" />
        <PolarAngleAxis dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 11 }} />
        <PolarRadiusAxis domain={[0, 100]} tick={false} axisLine={false} />
        <Radar
          dataKey="value"
          stroke={color}
          fill={color}
          fillOpacity={fillOpacity}
          strokeWidth={2}
        />
      </RechartsRadar>
    </ResponsiveContainer>
  );
}
