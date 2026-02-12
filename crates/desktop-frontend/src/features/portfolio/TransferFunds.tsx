import { useState } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  TextField,
  Button,
  Slider,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Chip,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  InputAdornment,
} from '@mui/material';
import SwapHorizIcon from '@mui/icons-material/SwapHoriz';
import { useAppStore } from '@/store/appStore';
import type { TransferRecord } from '@/store/appStore';
import { formatCurrency } from '@/utils/format';

const PRESETS = [
  { label: '20 / 80', pnc: 20, cap1: 80 },
  { label: '40 / 60', pnc: 40, cap1: 60 },
  { label: '50 / 50', pnc: 50, cap1: 50 },
  { label: '60 / 40', pnc: 60, cap1: 40 },
  { label: '80 / 20', pnc: 80, cap1: 20 },
];

export default function TransferFunds() {
  const bankAccounts = useAppStore((s) => s.bankAccounts);
  const transferHistory = useAppStore((s) => s.transferHistory);
  const addTransfer = useAppStore((s) => s.addTransfer);

  const [amount, setAmount] = useState('');
  const [splitPct, setSplitPct] = useState(40); // PNC pct; Cap1 = 100 - splitPct
  const [confirmOpen, setConfirmOpen] = useState(false);

  const pnc = bankAccounts.find((b) => b.id === 'pnc')!;
  const cap1 = bankAccounts.find((b) => b.id === 'cap1')!;

  const numAmount = Math.max(0, parseFloat(amount) || 0);
  const pncAmount = Math.round(numAmount * splitPct) / 100;
  const cap1Amount = numAmount - pncAmount;

  const handlePreset = (pncPct: number) => setSplitPct(pncPct);

  const handleConfirm = () => {
    if (numAmount <= 0) return;

    const record: TransferRecord = {
      id: crypto.randomUUID(),
      date: new Date().toISOString(),
      totalAmount: numAmount,
      splits: [
        { bankId: 'pnc', bankName: pnc.name, amount: pncAmount, pct: splitPct },
        { bankId: 'cap1', bankName: cap1.name, amount: cap1Amount, pct: 100 - splitPct },
      ],
    };

    addTransfer(record);
    setAmount('');
    setConfirmOpen(false);
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <Card>
        <CardContent>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
            <SwapHorizIcon sx={{ color: 'primary.main' }} />
            <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
              Transfer Funds
            </Typography>
          </Box>

          <TextField
            label="Transfer Amount"
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            fullWidth
            size="small"
            sx={{ mb: 2 }}
            slotProps={{
              input: {
                startAdornment: <InputAdornment position="start">$</InputAdornment>,
              },
            }}
          />

          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            Split Ratio (PNC / Capital One)
          </Typography>

          <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap' }}>
            {PRESETS.map((p) => (
              <Chip
                key={p.label}
                label={p.label}
                size="small"
                variant={splitPct === p.pnc ? 'filled' : 'outlined'}
                color={splitPct === p.pnc ? 'primary' : 'default'}
                onClick={() => handlePreset(p.pnc)}
                sx={{ cursor: 'pointer' }}
              />
            ))}
          </Box>

          <Box sx={{ px: 1 }}>
            <Slider
              value={splitPct}
              onChange={(_, v) => setSplitPct(v as number)}
              min={0}
              max={100}
              step={5}
              valueLabelDisplay="auto"
              valueLabelFormat={(v) => `${v}%`}
              sx={{
                '& .MuiSlider-track': { background: pnc.color },
                '& .MuiSlider-rail': { background: cap1.color, opacity: 0.5 },
              }}
            />
          </Box>

          {numAmount > 0 && (
            <Box
              sx={{
                display: 'flex',
                justifyContent: 'space-between',
                bgcolor: 'rgba(102, 126, 234, 0.08)',
                borderRadius: 1,
                p: 1.5,
                mt: 1,
                mb: 2,
              }}
            >
              <Box>
                <Typography variant="caption" sx={{ color: pnc.color, fontWeight: 600 }}>
                  {pnc.name}
                </Typography>
                <Typography variant="body2" sx={{ fontWeight: 600 }}>
                  {formatCurrency(pncAmount)} ({splitPct}%)
                </Typography>
              </Box>
              <Box sx={{ textAlign: 'right' }}>
                <Typography variant="caption" sx={{ color: cap1.color, fontWeight: 600 }}>
                  {cap1.name}
                </Typography>
                <Typography variant="body2" sx={{ fontWeight: 600 }}>
                  {formatCurrency(cap1Amount)} ({100 - splitPct}%)
                </Typography>
              </Box>
            </Box>
          )}

          <Button
            variant="contained"
            fullWidth
            disabled={numAmount <= 0}
            onClick={() => setConfirmOpen(true)}
          >
            Transfer {numAmount > 0 ? formatCurrency(numAmount) : ''}
          </Button>
        </CardContent>
      </Card>

      <Dialog open={confirmOpen} onClose={() => setConfirmOpen(false)} maxWidth="xs" fullWidth>
        <DialogTitle>Confirm Transfer</DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ mb: 1 }}>
            Transfer <strong>{formatCurrency(numAmount)}</strong> from brokerage:
          </Typography>
          <Typography variant="body2" sx={{ color: pnc.color }}>
            {pnc.name}: {formatCurrency(pncAmount)} ({splitPct}%)
          </Typography>
          <Typography variant="body2" sx={{ color: cap1.color }}>
            {cap1.name}: {formatCurrency(cap1Amount)} ({100 - splitPct}%)
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setConfirmOpen(false)}>Cancel</Button>
          <Button variant="contained" onClick={handleConfirm}>
            Confirm
          </Button>
        </DialogActions>
      </Dialog>

      {transferHistory.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="subtitle1" sx={{ fontWeight: 600, mb: 2 }}>
              Transfer History
            </Typography>
            <TableContainer component={Paper} sx={{ bgcolor: 'transparent' }}>
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell sx={{ color: 'text.secondary', fontWeight: 600, borderColor: 'divider' }}>Date</TableCell>
                    <TableCell align="right" sx={{ color: 'text.secondary', fontWeight: 600, borderColor: 'divider' }}>Total</TableCell>
                    <TableCell align="right" sx={{ color: 'text.secondary', fontWeight: 600, borderColor: 'divider' }}>PNC</TableCell>
                    <TableCell align="right" sx={{ color: 'text.secondary', fontWeight: 600, borderColor: 'divider' }}>Capital One</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {transferHistory.map((t) => {
                    const pncSplit = t.splits.find((s) => s.bankId === 'pnc');
                    const cap1Split = t.splits.find((s) => s.bankId === 'cap1');
                    return (
                      <TableRow key={t.id} hover sx={{ '&:hover': { bgcolor: 'rgba(102, 126, 234, 0.05)' } }}>
                        <TableCell sx={{ borderColor: 'divider' }}>
                          {new Date(t.date).toLocaleDateString()}
                        </TableCell>
                        <TableCell align="right" sx={{ borderColor: 'divider' }}>
                          {formatCurrency(t.totalAmount)}
                        </TableCell>
                        <TableCell align="right" sx={{ borderColor: 'divider' }}>
                          {pncSplit ? `${formatCurrency(pncSplit.amount)} (${pncSplit.pct}%)` : '-'}
                        </TableCell>
                        <TableCell align="right" sx={{ borderColor: 'divider' }}>
                          {cap1Split ? `${formatCurrency(cap1Split.amount)} (${cap1Split.pct}%)` : '-'}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          </CardContent>
        </Card>
      )}
    </Box>
  );
}
