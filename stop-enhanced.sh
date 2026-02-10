#!/bin/bash

# InvestIQ Enhanced Dashboard Stopper
# Gracefully stops all enhanced dashboards

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo ""
echo "═══════════════════════════════════════════════"
echo "  InvestIQ Enhanced Dashboard Stopper"
echo "═══════════════════════════════════════════════"
echo ""

# Check for PID file
if [ -f .dashboards.pid ]; then
    echo -e "${BLUE}→${NC} Found running dashboards..."
    echo ""

    while read pid; do
        if ps -p $pid > /dev/null 2>&1; then
            # Get process name
            PROC_NAME=$(ps -p $pid -o comm= 2>/dev/null || echo "Unknown")
            echo -e "${YELLOW}→${NC} Stopping $PROC_NAME (PID: $pid)"
            kill $pid 2>/dev/null

            # Wait for process to stop
            for i in {1..5}; do
                if ! ps -p $pid > /dev/null 2>&1; then
                    echo -e "${GREEN}✓${NC} Stopped successfully"
                    break
                fi
                sleep 1
            done

            # Force kill if still running
            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${YELLOW}⚠${NC}  Force stopping..."
                kill -9 $pid 2>/dev/null
                echo -e "${GREEN}✓${NC} Force stopped"
            fi
        else
            echo -e "${BLUE}i${NC} Process $pid already stopped"
        fi
        echo ""
    done < .dashboards.pid

    rm .dashboards.pid
    echo -e "${GREEN}✓${NC} All dashboards stopped"
else
    echo -e "${YELLOW}⚠${NC}  No running dashboards found (.dashboards.pid missing)"
    echo ""
    echo -e "${BLUE}→${NC} Searching for python dashboard processes..."

    # Try to find and kill by name
    FOUND=0

    # Look for analysis dashboard
    PIDS=$(pgrep -f "app_enhanced.py" || true)
    if [ ! -z "$PIDS" ]; then
        echo -e "${YELLOW}→${NC} Found analysis dashboard processes: $PIDS"
        echo $PIDS | xargs kill 2>/dev/null || true
        FOUND=1
    fi

    # Look for trading dashboard
    PIDS=$(pgrep -f "trading_dashboard_enhanced.py" || true)
    if [ ! -z "$PIDS" ]; then
        echo -e "${YELLOW}→${NC} Found trading dashboard processes: $PIDS"
        echo $PIDS | xargs kill 2>/dev/null || true
        FOUND=1
    fi

    if [ $FOUND -eq 1 ]; then
        echo -e "${GREEN}✓${NC} Stopped found processes"
    else
        echo -e "${BLUE}i${NC} No dashboard processes found"
    fi
fi

echo ""
echo "═══════════════════════════════════════════════"
echo ""
echo -e "${GREEN}Done!${NC} All enhanced dashboards have been stopped."
echo ""
echo "To start again, run: ${BLUE}./start-enhanced.sh${NC}"
echo ""
