#!/bin/bash

# InvestIQ Enhanced Dashboard Launcher
# Starts all enhanced dashboards with improved UX/UI

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Banner
echo ""
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                                                                ║"
echo "║               InvestIQ Enhanced Dashboard Launcher            ║"
echo "║                                                                ║"
echo "║               🚀 Next-Generation Stock Analysis               ║"
echo "║                                                                ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# Check if API server is running
echo -e "${BLUE}[1/5]${NC} Checking API server..."
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} API server is running"
else
    echo -e "${YELLOW}⚠${NC}  API server is not running"
    echo -e "${YELLOW}→${NC}  Starting API server in background..."

    # Check if we can start it
    if [ -f "Cargo.toml" ]; then
        cargo run --release --bin api-server > api-server.log 2>&1 &
        API_PID=$!
        echo -e "${GREEN}✓${NC} API server started (PID: $API_PID)"
        sleep 3
    else
        echo -e "${RED}✗${NC} Cannot start API server (Cargo.toml not found)"
        echo -e "${YELLOW}→${NC}  Please start the API server manually:"
        echo -e "   ${BLUE}cargo run --release --bin api-server${NC}"
        exit 1
    fi
fi

# Check Python dependencies
echo ""
echo -e "${BLUE}[2/5]${NC} Checking Python dependencies..."
MISSING_DEPS=0

for pkg in dash dash_bootstrap_components plotly pandas requests; do
    if ! python3 -c "import $pkg" 2>/dev/null; then
        echo -e "${RED}✗${NC} Missing: $pkg"
        MISSING_DEPS=1
    else
        echo -e "${GREEN}✓${NC} Found: $pkg"
    fi
done

if [ $MISSING_DEPS -eq 1 ]; then
    echo ""
    echo -e "${YELLOW}⚠${NC}  Some dependencies are missing"
    echo -e "${YELLOW}→${NC}  Install with: ${BLUE}pip install dash dash-bootstrap-components plotly pandas requests${NC}"
    read -p "Install now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        pip install dash dash-bootstrap-components plotly pandas requests
        echo -e "${GREEN}✓${NC} Dependencies installed"
    else
        echo -e "${RED}✗${NC} Cannot continue without dependencies"
        exit 1
    fi
fi

# Check for enhanced files
echo ""
echo -e "${BLUE}[3/5]${NC} Checking enhanced dashboard files..."
if [ ! -f "frontend/app_enhanced.py" ]; then
    echo -e "${RED}✗${NC} Enhanced dashboard not found"
    echo -e "${YELLOW}→${NC}  Expected: frontend/app_enhanced.py"
    exit 1
else
    echo -e "${GREEN}✓${NC} Enhanced analysis dashboard found"
fi

if [ ! -f "frontend/trading_dashboard_enhanced.py" ]; then
    echo -e "${YELLOW}⚠${NC}  Enhanced trading dashboard not found"
    echo -e "${YELLOW}→${NC}  Only analysis dashboard will be started"
    TRADING_AVAILABLE=0
else
    echo -e "${GREEN}✓${NC} Enhanced trading dashboard found"
    TRADING_AVAILABLE=1
fi

if [ ! -f "frontend/assets/enhanced.css" ]; then
    echo -e "${YELLOW}⚠${NC}  Enhanced CSS not found"
    echo -e "${YELLOW}→${NC}  Dashboard will use default styles"
else
    echo -e "${GREEN}✓${NC} Enhanced CSS found"
fi

# Check API key for trading dashboard
echo ""
echo -e "${BLUE}[4/5]${NC} Checking configuration..."
if [ -z "$API_KEY" ] && [ $TRADING_AVAILABLE -eq 1 ]; then
    echo -e "${YELLOW}⚠${NC}  API_KEY not set"

    # Check .env file
    if [ -f ".env" ] && grep -q "API_KEYS=" .env; then
        API_KEY=$(grep "API_KEYS=" .env | head -1 | cut -d '=' -f2 | cut -d ',' -f1)
        export API_KEY
        echo -e "${GREEN}✓${NC} API_KEY loaded from .env"
    else
        echo -e "${YELLOW}→${NC}  Trading dashboard requires API_KEY"
        echo -e "${YELLOW}→${NC}  You can set it with: ${BLUE}export API_KEY=your_key_here${NC}"
        echo ""
        read -p "Continue without trading dashboard? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
        TRADING_AVAILABLE=0
    fi
else
    echo -e "${GREEN}✓${NC} Configuration ready"
fi

# Create log directory
mkdir -p logs

# Start dashboards
echo ""
echo -e "${BLUE}[5/5]${NC} Starting enhanced dashboards..."
echo ""

# Start analysis dashboard
echo -e "${GREEN}→${NC} Starting Enhanced Analysis Dashboard..."
cd frontend
python3 app_enhanced.py > ../logs/analysis-dashboard.log 2>&1 &
ANALYSIS_PID=$!
cd ..
sleep 2

# Check if it started
if ps -p $ANALYSIS_PID > /dev/null; then
    echo -e "${GREEN}✓${NC} Analysis Dashboard running (PID: $ANALYSIS_PID)"
    echo -e "${BLUE}  →${NC} http://localhost:8050"
else
    echo -e "${RED}✗${NC} Failed to start Analysis Dashboard"
    echo -e "${YELLOW}  →${NC} Check logs/analysis-dashboard.log for errors"
fi

# Start trading dashboard if available
if [ $TRADING_AVAILABLE -eq 1 ]; then
    echo ""
    echo -e "${GREEN}→${NC} Starting Enhanced Trading Dashboard..."
    cd frontend
    API_KEY=$API_KEY python3 trading_dashboard_enhanced.py > ../logs/trading-dashboard.log 2>&1 &
    TRADING_PID=$!
    cd ..
    sleep 2

    # Check if it started
    if ps -p $TRADING_PID > /dev/null; then
        echo -e "${GREEN}✓${NC} Trading Dashboard running (PID: $TRADING_PID)"
        echo -e "${BLUE}  →${NC} http://localhost:8052"
    else
        echo -e "${RED}✗${NC} Failed to start Trading Dashboard"
        echo -e "${YELLOW}  →${NC} Check logs/trading-dashboard.log for errors"
    fi
fi

# Save PIDs for cleanup
echo "$ANALYSIS_PID" > .dashboards.pid
if [ $TRADING_AVAILABLE -eq 1 ]; then
    echo "$TRADING_PID" >> .dashboards.pid
fi

# Summary
echo ""
echo "════════════════════════════════════════════════════════════════"
echo ""
echo -e "${GREEN}✨ InvestIQ Enhanced Dashboards Started Successfully! ✨${NC}"
echo ""
echo "📊 Access Points:"
echo "   Analysis Dashboard:  http://localhost:8050"
if [ $TRADING_AVAILABLE -eq 1 ]; then
    echo "   Trading Dashboard:   http://localhost:8052"
fi
echo "   API Server:          http://localhost:3000"
echo ""
echo "📝 Logs:"
echo "   Analysis: logs/analysis-dashboard.log"
if [ $TRADING_AVAILABLE -eq 1 ]; then
    echo "   Trading:  logs/trading-dashboard.log"
fi
echo ""
echo "🎯 Quick Tips:"
echo "   • Press '/' to quickly search for stocks"
echo "   • Press 'H' for keyboard shortcuts help"
echo "   • Press 'W' to toggle your watchlist"
echo "   • Check the welcome tour for first-time guidance"
echo ""
echo "⚙️  Configuration:"
echo "   • Settings: Click gear icon in navbar"
echo "   • Theme: Toggle dark/light mode"
echo "   • Auto-refresh: Enable in settings or navbar"
echo ""
echo "📚 Documentation:"
echo "   • UX Guide:         frontend/UX_IMPROVEMENTS.md"
echo "   • Quick Reference:  frontend/QUICK_REFERENCE.md"
echo "   • Migration Guide:  frontend/MIGRATION_GUIDE.md"
echo ""
echo "🛑 To stop all dashboards:"
echo "   ${BLUE}./stop-enhanced.sh${NC}"
echo "   or press Ctrl+C in each terminal"
echo ""
echo "════════════════════════════════════════════════════════════════"
echo ""

# Wait for user interrupt
echo -e "${YELLOW}Press Ctrl+C to stop all dashboards...${NC}"
echo ""

# Trap Ctrl+C to cleanup
cleanup() {
    echo ""
    echo ""
    echo -e "${YELLOW}Shutting down...${NC}"

    if [ -f .dashboards.pid ]; then
        while read pid; do
            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${BLUE}→${NC} Stopping process $pid"
                kill $pid 2>/dev/null || true
            fi
        done < .dashboards.pid
        rm .dashboards.pid
    fi

    echo -e "${GREEN}✓${NC} All dashboards stopped"
    echo ""
    exit 0
}

trap cleanup INT TERM

# Keep script running
while true; do
    sleep 1
done
