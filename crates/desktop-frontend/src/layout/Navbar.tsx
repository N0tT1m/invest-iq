import { AppBar, Toolbar, IconButton, TextField, InputAdornment, Box, Chip } from '@mui/material';
import { Menu as MenuIcon, Search as SearchIcon, Settings as SettingsIcon } from '@mui/icons-material';
import { useState, type KeyboardEvent } from 'react';
import { useAppStore } from '@/store/appStore';
import { DRAWER_WIDTH } from './Sidebar';

export default function Navbar() {
  const { symbol, setSymbol, sidebarOpen, toggleSidebar } = useAppStore();
  const [input, setInput] = useState(symbol);

  const handleSubmit = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && input.trim()) {
      setSymbol(input.trim());
    }
  };

  return (
    <AppBar
      position="fixed"
      elevation={0}
      sx={{
        bgcolor: 'rgba(26, 31, 58, 0.95)',
        backdropFilter: 'blur(10px)',
        borderBottom: '1px solid',
        borderColor: 'divider',
        ml: sidebarOpen ? `${DRAWER_WIDTH}px` : 0,
        width: sidebarOpen ? `calc(100% - ${DRAWER_WIDTH}px)` : '100%',
        transition: 'margin 225ms cubic-bezier(0,0,0.2,1), width 225ms cubic-bezier(0,0,0.2,1)',
      }}
    >
      <Toolbar sx={{ gap: 2 }}>
        <IconButton edge="start" color="inherit" onClick={toggleSidebar}>
          <MenuIcon />
        </IconButton>

        <TextField
          size="small"
          placeholder="Search symbol... (e.g. AAPL)"
          value={input}
          onChange={(e) => setInput(e.target.value.toUpperCase())}
          onKeyDown={handleSubmit}
          slotProps={{
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchIcon sx={{ color: 'text.secondary', fontSize: 20 }} />
                </InputAdornment>
              ),
            },
          }}
          sx={{
            width: 280,
            '& .MuiOutlinedInput-root': {
              bgcolor: 'rgba(255,255,255,0.05)',
              borderRadius: 2,
            },
          }}
        />

        {symbol && (
          <Chip label={symbol} color="primary" variant="outlined" size="small" />
        )}

        <Box sx={{ flexGrow: 1 }} />

        <IconButton color="inherit">
          <SettingsIcon />
        </IconButton>
      </Toolbar>
    </AppBar>
  );
}
