import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Box,
  Typography,
  IconButton,
  InputAdornment,
} from '@mui/material';
import { Visibility, VisibilityOff } from '@mui/icons-material';
import { useAppStore } from '@/store/appStore';

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function SettingsDialog({ open, onClose }: Props) {
  const { apiKey, liveTradingKey, setApiKey, setLiveTradingKey } = useAppStore();

  const [localApiKey, setLocalApiKey] = useState(apiKey);
  const [localLiveKey, setLocalLiveKey] = useState(liveTradingKey);
  const [showApiKey, setShowApiKey] = useState(false);
  const [showLiveKey, setShowLiveKey] = useState(false);

  useEffect(() => {
    if (open) {
      setLocalApiKey(apiKey);
      setLocalLiveKey(liveTradingKey);
    }
  }, [open, apiKey, liveTradingKey]);

  const handleSave = () => {
    setApiKey(localApiKey.trim());
    setLiveTradingKey(localLiveKey.trim());
    onClose();
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle>Settings</DialogTitle>
      <DialogContent>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3, mt: 1 }}>
          <Box>
            <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>
              API Key
            </Typography>
            <TextField
              fullWidth
              size="small"
              placeholder="Enter your API key"
              type={showApiKey ? 'text' : 'password'}
              value={localApiKey}
              onChange={(e) => setLocalApiKey(e.target.value)}
              slotProps={{
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      <IconButton size="small" onClick={() => setShowApiKey(!showApiKey)}>
                        {showApiKey ? <VisibilityOff fontSize="small" /> : <Visibility fontSize="small" />}
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
            <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5 }}>
              Sent as x-api-key header with all requests.
            </Typography>
          </Box>

          <Box>
            <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>
              Live Trading Key
            </Typography>
            <TextField
              fullWidth
              size="small"
              placeholder="Enter your live trading key"
              type={showLiveKey ? 'text' : 'password'}
              value={localLiveKey}
              onChange={(e) => setLocalLiveKey(e.target.value)}
              slotProps={{
                input: {
                  endAdornment: (
                    <InputAdornment position="end">
                      <IconButton size="small" onClick={() => setShowLiveKey(!showLiveKey)}>
                        {showLiveKey ? <VisibilityOff fontSize="small" /> : <Visibility fontSize="small" />}
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            />
            <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5 }}>
              Required for live trading. Sent as x-live-trading-key header.
            </Typography>
          </Box>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="contained" onClick={handleSave}>
          Save
        </Button>
      </DialogActions>
    </Dialog>
  );
}
