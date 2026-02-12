import { lazy, Suspense } from 'react';
import { Routes, Route } from 'react-router-dom';
import AppShell from '@/layout/AppShell';
import ErrorBoundary from '@/components/ErrorBoundary';
import LoadingOverlay from '@/components/LoadingOverlay';

const DashboardPage = lazy(() => import('@/pages/DashboardPage'));
const SearchPage = lazy(() => import('@/pages/SearchPage'));
const ScreenerPage = lazy(() => import('@/pages/ScreenerPage'));
const TradingPage = lazy(() => import('@/pages/TradingPage'));
const PortfolioPage = lazy(() => import('@/pages/PortfolioPage'));
const ResearchPage = lazy(() => import('@/pages/ResearchPage'));
const IntelligencePage = lazy(() => import('@/pages/IntelligencePage'));
const MLPage = lazy(() => import('@/pages/MLPage'));

export default function App() {
  return (
    <ErrorBoundary>
      <Suspense fallback={<LoadingOverlay />}>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/search" element={<SearchPage />} />
            <Route path="/screener" element={<ScreenerPage />} />
            <Route path="/trading" element={<TradingPage />} />
            <Route path="/portfolio" element={<PortfolioPage />} />
            <Route path="/research" element={<ResearchPage />} />
            <Route path="/intelligence" element={<IntelligencePage />} />
            <Route path="/ml" element={<MLPage />} />
          </Route>
        </Routes>
      </Suspense>
    </ErrorBoundary>
  );
}
