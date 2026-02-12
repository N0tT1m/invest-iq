import { useState } from 'react';
import {
  Box,
  TextField,
  Button,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  InputAdornment,
} from '@mui/material';
import { Search as SearchIcon, Analytics as AnalyzeIcon } from '@mui/icons-material';
import { useAppStore } from '@/store/appStore';

const TIMEFRAMES = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w', '1M'];
const DAY_PRESETS = [30, 90, 180, 365, 730];

export default function SymbolInput() {
  const { symbol, timeframe, daysBack, setSymbol, setTimeframe, setDaysBack } = useAppStore();
  const [input, setInput] = useState(symbol);

  const handleAnalyze = () => {
    if (input.trim()) setSymbol(input.trim());
  };

  return (
    <Box sx={{ display: 'flex', gap: 2, alignItems: 'center', flexWrap: 'wrap' }}>
      <TextField
        size="small"
        placeholder="Enter symbol..."
        value={input}
        onChange={(e) => setInput(e.target.value.toUpperCase())}
        onKeyDown={(e) => e.key === 'Enter' && handleAnalyze()}
        slotProps={{
          input: {
            startAdornment: (
              <InputAdornment position="start">
                <SearchIcon sx={{ fontSize: 20, color: 'text.secondary' }} />
              </InputAdornment>
            ),
          },
        }}
        sx={{ width: 180 }}
      />

      <FormControl size="small" sx={{ minWidth: 100 }}>
        <InputLabel>Timeframe</InputLabel>
        <Select value={timeframe} label="Timeframe" onChange={(e) => setTimeframe(e.target.value)}>
          {TIMEFRAMES.map((tf) => (
            <MenuItem key={tf} value={tf}>{tf}</MenuItem>
          ))}
        </Select>
      </FormControl>

      <FormControl size="small" sx={{ minWidth: 100 }}>
        <InputLabel>Days</InputLabel>
        <Select value={daysBack} label="Days" onChange={(e) => setDaysBack(Number(e.target.value))}>
          {DAY_PRESETS.map((d) => (
            <MenuItem key={d} value={d}>{d}d</MenuItem>
          ))}
        </Select>
      </FormControl>

      <Button
        variant="contained"
        startIcon={<AnalyzeIcon />}
        onClick={handleAnalyze}
        disabled={!input.trim()}
        sx={{
          background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
          '&:hover': { background: 'linear-gradient(135deg, #7b8ff0 0%, #8a5bb5 100%)' },
        }}
      >
        Analyze
      </Button>
    </Box>
  );
}
