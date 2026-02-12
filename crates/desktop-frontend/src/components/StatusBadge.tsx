import { Chip, type ChipProps } from '@mui/material';
import { signalColor } from '@/utils/format';

interface StatusBadgeProps {
  signal: string;
  size?: ChipProps['size'];
}

export default function StatusBadge({ signal, size = 'small' }: StatusBadgeProps) {
  const color = signalColor(signal);
  const label = signal.replace(/_/g, ' ').toUpperCase();

  return (
    <Chip
      label={label}
      size={size}
      sx={{
        bgcolor: `${color}22`,
        color,
        border: `1px solid ${color}44`,
        fontWeight: 700,
      }}
    />
  );
}
