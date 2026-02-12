import { useState } from 'react';
import {
  Box, Card, CardContent, Typography, Button,
  FormControl, InputLabel, Select, MenuItem, TextField, Alert,
} from '@mui/material';
import { FilterList as FilterIcon } from '@mui/icons-material';
import StatusBadge from '@/components/StatusBadge';
import DataTable from '@/components/DataTable';
import { useScanStocks, useScreenerPresets } from '@/hooks/useSearch';
import { useAppStore } from '@/store/appStore';
import { useNavigate } from 'react-router-dom';
import { formatCurrency, formatPercent } from '@/utils/format';
import type { ScreenerResult } from '@/api/types';

export default function StockScreener() {
  const [preset, setPreset] = useState('');
  const [minPrice, setMinPrice] = useState('');
  const [maxPrice, setMaxPrice] = useState('');
  const [signal, setSignal] = useState('');

  const presets = useScreenerPresets();
  const scan = useScanStocks();
  const setSymbol = useAppStore((s) => s.setSymbol);
  const navigate = useNavigate();

  const handleScan = () => {
    const filters: Record<string, unknown> = {};
    if (preset) filters.preset = preset;
    if (minPrice) filters.min_price = Number(minPrice);
    if (maxPrice) filters.max_price = Number(maxPrice);
    if (signal) filters.signal = signal;
    scan.mutate(filters);
  };

  const handleSelect = (symbol: string) => {
    setSymbol(symbol);
    navigate('/');
  };

  const columns = [
    {
      key: 'symbol', header: 'Symbol',
      render: (r: ScreenerResult) => (
        <Typography
          variant="body2"
          sx={{ fontWeight: 700, cursor: 'pointer', '&:hover': { color: 'primary.main' } }}
          onClick={() => handleSelect(r.symbol)}
        >
          {r.symbol}
        </Typography>
      ),
    },
    { key: 'name', header: 'Name' },
    { key: 'price', header: 'Price', align: 'right' as const, render: (r: ScreenerResult) => r.price != null ? formatCurrency(r.price) : '-' },
    {
      key: 'change_pct', header: 'Change', align: 'right' as const,
      render: (r: ScreenerResult) => r.change_pct != null ? (
        <Typography variant="body2" sx={{ color: r.change_pct >= 0 ? '#00cc88' : '#ff4444' }}>
          {formatPercent(r.change_pct)}
        </Typography>
      ) : '-',
    },
    {
      key: 'signal', header: 'Signal',
      render: (r: ScreenerResult) => r.signal ? <StatusBadge signal={r.signal} /> : '-',
    },
  ];

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <Card>
        <CardContent>
          <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>Filters</Typography>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
            <FormControl size="small" sx={{ minWidth: 140 }}>
              <InputLabel>Preset</InputLabel>
              <Select value={preset} label="Preset" onChange={(e) => setPreset(e.target.value)}>
                <MenuItem value="">None</MenuItem>
                {(presets.data ?? []).map((p, i) => (
                  <MenuItem key={i} value={String(p.name ?? p.id ?? i)}>{String(p.name ?? `Preset ${i + 1}`)}</MenuItem>
                ))}
              </Select>
            </FormControl>
            <TextField size="small" label="Min Price" type="number" value={minPrice} onChange={(e) => setMinPrice(e.target.value)} sx={{ width: 100 }} />
            <TextField size="small" label="Max Price" type="number" value={maxPrice} onChange={(e) => setMaxPrice(e.target.value)} sx={{ width: 100 }} />
            <FormControl size="small" sx={{ minWidth: 120 }}>
              <InputLabel>Signal</InputLabel>
              <Select value={signal} label="Signal" onChange={(e) => setSignal(e.target.value)}>
                <MenuItem value="">Any</MenuItem>
                <MenuItem value="strong_buy">Strong Buy</MenuItem>
                <MenuItem value="buy">Buy</MenuItem>
                <MenuItem value="neutral">Neutral</MenuItem>
                <MenuItem value="sell">Sell</MenuItem>
                <MenuItem value="strong_sell">Strong Sell</MenuItem>
              </Select>
            </FormControl>
            <Button
              variant="contained"
              startIcon={<FilterIcon />}
              onClick={handleScan}
              disabled={scan.isPending}
            >
              {scan.isPending ? 'Scanning...' : 'Scan'}
            </Button>
          </Box>
        </CardContent>
      </Card>

      {scan.error && <Alert severity="error">{(scan.error as Error).message}</Alert>}

      {scan.data && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>
              Results ({scan.data.length} stocks)
            </Typography>
            <DataTable columns={columns} data={scan.data} emptyMessage="No stocks matched your criteria" />
          </CardContent>
        </Card>
      )}
    </Box>
  );
}
