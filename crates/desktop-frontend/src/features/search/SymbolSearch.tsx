import { useState } from 'react';
import {
  Box, TextField, InputAdornment, Card, CardContent, CardActionArea,
  Typography, Chip, CircularProgress,
} from '@mui/material';
import { Search as SearchIcon } from '@mui/icons-material';
import { useSymbolSearch } from '@/hooks/useSearch';
import { useAppStore } from '@/store/appStore';
import { useNavigate } from 'react-router-dom';

export default function SymbolSearch() {
  const [query, setQuery] = useState('');
  const { data, isLoading } = useSymbolSearch(query);
  const setSymbol = useAppStore((s) => s.setSymbol);
  const navigate = useNavigate();

  const handleSelect = (symbol: string) => {
    setSymbol(symbol);
    navigate('/');
  };

  return (
    <Box>
      <TextField
        fullWidth
        size="medium"
        placeholder="Search for a symbol, company name..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        slotProps={{
          input: {
            startAdornment: (
              <InputAdornment position="start">
                <SearchIcon />
              </InputAdornment>
            ),
            endAdornment: isLoading ? (
              <InputAdornment position="end">
                <CircularProgress size={20} />
              </InputAdornment>
            ) : null,
          },
        }}
        sx={{ mb: 3, '& .MuiOutlinedInput-root': { bgcolor: 'rgba(255,255,255,0.05)', borderRadius: 2 } }}
      />

      {data && data.length > 0 && (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          {data.map((r) => (
            <Card key={r.symbol}>
              <CardActionArea onClick={() => handleSelect(r.symbol)}>
                <CardContent sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', py: 1.5 }}>
                  <Box>
                    <Typography variant="body1" sx={{ fontWeight: 700 }}>{r.symbol}</Typography>
                    <Typography variant="body2" color="text.secondary">{r.name}</Typography>
                  </Box>
                  <Box sx={{ display: 'flex', gap: 1 }}>
                    {r.exchange && <Chip label={r.exchange} size="small" variant="outlined" />}
                    {r.type && <Chip label={r.type} size="small" variant="outlined" />}
                  </Box>
                </CardContent>
              </CardActionArea>
            </Card>
          ))}
        </Box>
      )}

      {data && data.length === 0 && query.length >= 1 && (
        <Typography variant="body2" color="text.secondary" sx={{ textAlign: 'center', py: 4 }}>
          No results found for "{query}"
        </Typography>
      )}
    </Box>
  );
}
