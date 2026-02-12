import { useState } from 'react';
import { Box, Tabs, Tab } from '@mui/material';
import PaperTrading from './PaperTrading';
import BacktestPanel from './BacktestPanel';
import LiveTrading from './LiveTrading';
import AgentTrades from './AgentTrades';
import AgentAnalytics from './AgentAnalytics';

const TABS = [
  { label: 'Paper Trade', key: 'paper' },
  { label: 'Backtest', key: 'backtest' },
  { label: 'Live Trade', key: 'live' },
  { label: 'Agent Trades', key: 'agent' },
  { label: 'Agent Analytics', key: 'analytics' },
];

export default function TradingTabs() {
  const [tab, setTab] = useState(0);

  return (
    <Box>
      <Tabs
        value={tab}
        onChange={(_, v) => setTab(v)}
        variant="scrollable"
        scrollButtons="auto"
        sx={{ borderBottom: 1, borderColor: 'divider', mb: 3 }}
      >
        {TABS.map((t) => (
          <Tab key={t.key} label={t.label} />
        ))}
      </Tabs>

      {tab === 0 && <PaperTrading />}
      {tab === 1 && <BacktestPanel />}
      {tab === 2 && <LiveTrading />}
      {tab === 3 && <AgentTrades />}
      {tab === 4 && <AgentAnalytics />}
    </Box>
  );
}
