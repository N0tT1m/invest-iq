import { Box, Toolbar } from '@mui/material';
import { Outlet } from 'react-router-dom';
import Sidebar, { DRAWER_WIDTH } from './Sidebar';
import Navbar from './Navbar';
import { useAppStore } from '@/store/appStore';

export default function AppShell() {
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);

  return (
    <Box sx={{ display: 'flex', minHeight: '100vh', bgcolor: 'background.default' }}>
      <Navbar />
      <Sidebar open={sidebarOpen} />
      <Box
        component="main"
        sx={{
          flexGrow: 1,
          ml: sidebarOpen ? `${DRAWER_WIDTH}px` : 0,
          transition: 'margin 225ms cubic-bezier(0,0,0.2,1)',
          p: 3,
          width: '100%',
        }}
      >
        <Toolbar />
        <Outlet />
      </Box>
    </Box>
  );
}
