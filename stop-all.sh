#!/bin/bash

# InvestIQ - Stop Everything Script

echo "======================================"
echo "🛑 InvestIQ - Stopping All Services"
echo "======================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Stop API Server
if [ -f .api-server.pid ]; then
    API_PID=$(cat .api-server.pid)
    echo "Stopping API Server (PID: $API_PID)..."
    kill $API_PID 2>/dev/null && echo -e "${GREEN}✅ API Server stopped${NC}" || echo -e "${YELLOW}⚠️  API Server not running${NC}"
    rm .api-server.pid
else
    # Try to kill by port
    if lsof -Pi :3000 -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo "Stopping API Server on port 3000..."
        kill $(lsof -t -i:3000) 2>/dev/null && echo -e "${GREEN}✅ API Server stopped${NC}"
    fi
fi

# Stop Dashboard
if [ -f .dashboard.pid ]; then
    DASH_PID=$(cat .dashboard.pid)
    echo "Stopping Dashboard (PID: $DASH_PID)..."
    kill $DASH_PID 2>/dev/null && echo -e "${GREEN}✅ Dashboard stopped${NC}" || echo -e "${YELLOW}⚠️  Dashboard not running${NC}"
    rm .dashboard.pid
else
    # Try to kill by port
    if lsof -Pi :8050 -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo "Stopping Dashboard on port 8050..."
        kill $(lsof -t -i:8050) 2>/dev/null && echo -e "${GREEN}✅ Dashboard stopped${NC}"
    fi
fi

# Stop Discord Bot (if running)
if pgrep -f "discord-bot" > /dev/null; then
    echo "Stopping Discord Bot..."
    pkill -f "discord-bot" && echo -e "${GREEN}✅ Discord Bot stopped${NC}"
fi

# Optionally stop Redis
read -p "Stop Redis container? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    if command -v docker &> /dev/null; then
        echo "Stopping Redis..."
        docker-compose down && echo -e "${GREEN}✅ Redis stopped${NC}"
    fi
fi

# Clean up log files (optional)
read -p "Delete log files? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -f api-server.log dashboard.log
    echo -e "${GREEN}✅ Log files deleted${NC}"
fi

echo ""
echo -e "${GREEN}======================================"
echo "✅ All services stopped"
echo "======================================${NC}"
