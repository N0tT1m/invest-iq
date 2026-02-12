import { Box, Typography } from '@mui/material';
import { SearchOff } from '@mui/icons-material';
import type { ReactNode } from 'react';

interface EmptyStateProps {
  title?: string;
  message?: string;
  icon?: ReactNode;
  action?: ReactNode;
}

export default function EmptyState({
  title = 'No data',
  message = 'Enter a symbol to get started.',
  icon,
  action,
}: EmptyStateProps) {
  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', py: 8, gap: 1.5 }}>
      <Box sx={{ color: 'text.secondary', opacity: 0.5, fontSize: 48 }}>
        {icon ?? <SearchOff sx={{ fontSize: 48 }} />}
      </Box>
      <Typography variant="h6" color="text.secondary">{title}</Typography>
      <Typography variant="body2" color="text.secondary">{message}</Typography>
      {action}
    </Box>
  );
}
