#!/bin/bash

# InvestIQ - Start Everything Script
# This script starts Redis, API server, and Dashboard

set -e  # Exit on error

echo "======================================"
echo "🚀 InvestIQ - Starting Full Stack"
echo "======================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if .env exists
if [ ! -f .env ]; then
    echo -e "${RED}❌ Error: .env file not found${NC}"
    echo "Please copy .env.example to .env and add your API keys:"
    echo "  cp .env.example .env"
    echo "  # Edit .env with your keys"
    exit 1
fi

# Load environment variables from .env
echo "Loading environment variables from .env..."
export $(grep -v '^#' .env | xargs)

# Set API_KEY from API_KEYS if not already set
if [ -z "$API_KEY" ]; then
    export API_KEY=$(echo $API_KEYS | cut -d',' -f1)
    echo -e "${GREEN}✅ API_KEY set from .env${NC}"
fi

# Check if Polygon API key is set
if [ -z "$POLYGON_API_KEY" ]; then
    echo -e "${YELLOW}⚠️  Warning: POLYGON_API_KEY not set in .env${NC}"
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "1️⃣  Starting Redis..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Check if docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${YELLOW}⚠️  Docker not found. Will use in-memory cache.${NC}"
else
    # Start Redis only (not the whole docker-compose stack)
    if docker ps | grep -q investiq-redis; then
        echo -e "${GREEN}✅ Redis already running${NC}"
    else
        echo "Starting Redis container..."
        docker run -d \
            --name investiq-redis \
            -p 6379:6379 \
            -v investiq-redis-data:/data \
            redis:7-alpine redis-server --appendonly yes
        sleep 2
        echo -e "${GREEN}✅ Redis started${NC}"
    fi
fi

echo ""
echo "2️⃣  Building Rust Backend..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Build if not already built
if [ ! -f target/release/api-server ]; then
    echo "Building API server (this may take a few minutes)..."
    cargo build --release
else
    echo -e "${GREEN}✅ API server already built${NC}"
fi

echo ""
echo "3️⃣  Starting API Server..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Kill existing API server if running
if lsof -Pi :3000 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "Killing existing API server on port 3000..."
    kill $(lsof -t -i:3000) 2>/dev/null || true
    sleep 1
fi

# Start API server in background
cargo run --release --bin api-server > api-server.log 2>&1 &
API_PID=$!
echo "API Server PID: $API_PID"

# Wait for API to be ready
echo "Waiting for API server to start..."
for i in {1..30}; do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ API server is ready!${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}❌ API server failed to start${NC}"
        echo "Check api-server.log for errors"
        exit 1
    fi
    sleep 1
done

echo ""
echo "4️⃣  Starting Dashboard..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}❌ Python 3 not found${NC}"
    echo "Please install Python 3.8 or higher"
    exit 1
fi

# Install Python dependencies if needed
cd frontend
if [ ! -d "venv" ]; then
    echo "Creating Python virtual environment..."
    python3 -m venv venv
fi

source venv/bin/activate

echo "Installing Python dependencies..."
pip install -q --upgrade pip
pip install -q -r requirements.txt

echo -e "${GREEN}✅ Starting Dashboard...${NC}"
python3 app.py > ../dashboard.log 2>&1 &
DASH_PID=$!

cd ..

# Wait for dashboard to be ready
echo "Waiting for dashboard to start..."
for i in {1..30}; do
    if curl -s http://localhost:8050 > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Dashboard is ready!${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}❌ Dashboard failed to start${NC}"
        echo "Check dashboard.log for errors"
        exit 1
    fi
    sleep 1
done

echo ""
echo "======================================"
echo "✅ InvestIQ is Running!"
echo "======================================"
echo ""
echo "📊 Dashboard:  http://localhost:8050"
echo "🔌 API Server: http://localhost:3000"
echo "💾 Redis:      localhost:6379"
echo ""
echo "Logs:"
echo "  API Server:  tail -f api-server.log"
echo "  Dashboard:   tail -f dashboard.log"
echo ""
echo "To stop all services:"
echo "  ./stop-all.sh"
echo "  or press Ctrl+C"
echo ""
echo "======================================"
echo ""

# Save PIDs for later
echo $API_PID > .api-server.pid
echo $DASH_PID > .dashboard.pid

# Keep script running and forward signals
trap 'echo ""; echo "Stopping services..."; ./stop-all.sh; exit 0' INT TERM

# Wait for both processes
wait $API_PID $DASH_PID
