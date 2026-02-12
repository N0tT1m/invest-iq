import { Box, Grid, Card, CardContent, Typography, Alert } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, ResponsiveContainer, Tooltip, PieChart, Pie, Cell } from 'recharts';
import MetricCard from '@/components/MetricCard';
import LoadingOverlay from '@/components/LoadingOverlay';
import { useAgentAnalyticsSummary, useSignalDistribution, useWinRateByRegime } from '@/hooks/useAgent';
import { formatPercent, formatCurrency } from '@/utils/format';
import { SERIES_PALETTE } from '@/theme/colors';

const CHART_TOOLTIP_STYLE = { background: '#1a1f3a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 8 };

export default function AgentAnalytics() {
  const summary = useAgentAnalyticsSummary();
  const signalDist = useSignalDistribution();
  const winByRegime = useWinRateByRegime();

  if (summary.isLoading) return <LoadingOverlay message="Loading agent analytics..." />;
  if (summary.error) return <Alert severity="error">{(summary.error as Error).message}</Alert>;

  const s = summary.data;

  const signalData = signalDist.data
    ? Object.entries(signalDist.data).map(([name, value]) => ({ name, value }))
    : [];

  const regimeData = winByRegime.data
    ? Object.entries(winByRegime.data).map(([name, value]) => ({ name, value: value * 100 }))
    : [];

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      {s && (
        <Grid container spacing={2}>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Total Trades" value={s.total_trades} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Approved" value={s.approved} color="#00cc88" />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Rejected" value={s.rejected} color="#ff4444" />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Win Rate" value={s.win_rate != null ? formatPercent(s.win_rate * 100) : 'N/A'} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Total P&L" value={s.total_pnl != null ? formatCurrency(s.total_pnl) : 'N/A'} color={s.total_pnl != null && s.total_pnl >= 0 ? '#00cc88' : '#ff4444'} />
          </Grid>
          <Grid size={{ xs: 6, md: 2 }}>
            <MetricCard title="Avg Confidence" value={s.avg_confidence != null ? formatPercent(s.avg_confidence * 100) : 'N/A'} />
          </Grid>
        </Grid>
      )}

      <Grid container spacing={2}>
        <Grid size={{ xs: 12, md: 6 }}>
          <Card>
            <CardContent>
              <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Signal Distribution</Typography>
              {signalData.length > 0 ? (
                <ResponsiveContainer width="100%" height={250}>
                  <PieChart>
                    <Pie data={signalData} dataKey="value" nameKey="name" innerRadius={50} outerRadius={80} paddingAngle={2}>
                      {signalData.map((_, i) => (
                        <Cell key={i} fill={SERIES_PALETTE[i % SERIES_PALETTE.length]} />
                      ))}
                    </Pie>
                    <Tooltip contentStyle={CHART_TOOLTIP_STYLE} />
                  </PieChart>
                </ResponsiveContainer>
              ) : (
                <Typography variant="body2" color="text.secondary">No data</Typography>
              )}
            </CardContent>
          </Card>
        </Grid>

        <Grid size={{ xs: 12, md: 6 }}>
          <Card>
            <CardContent>
              <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Win Rate by Regime</Typography>
              {regimeData.length > 0 ? (
                <ResponsiveContainer width="100%" height={250}>
                  <BarChart data={regimeData}>
                    <XAxis dataKey="name" tick={{ fill: '#a0a0a0', fontSize: 11 }} />
                    <YAxis tick={{ fill: '#a0a0a0', fontSize: 11 }} domain={[0, 100]} />
                    <Tooltip contentStyle={CHART_TOOLTIP_STYLE} formatter={(v) => `${Number(v).toFixed(1)}%`} />
                    <Bar dataKey="value" fill="#667eea" radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <Typography variant="body2" color="text.secondary">No data</Typography>
              )}
            </CardContent>
          </Card>
        </Grid>
      </Grid>
    </Box>
  );
}
