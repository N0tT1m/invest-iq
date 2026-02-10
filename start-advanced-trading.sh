#!/bin/bash
# Start all InvestIQ advanced trading services

set -e

echo "=================================================="
echo "InvestIQ Advanced Trading System"
echo "=================================================="
echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "ERROR: .env file not found"
    echo "Please copy .env.features.example to .env and configure it:"
    echo "  cp .env.features.example .env"
    echo "  vim .env  # Add your API keys"
    exit 1
fi

# Check Python installation
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 not found"
    echo "Please install Python 3.8 or later"
    exit 1
fi

# Function to start a service
start_service() {
    local name=$1
    local dir=$2
    local script=$3
    local port=$4

    echo "Starting $name on port $port..."

    # Check if port is already in use
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo "  WARNING: Port $port already in use. Skipping $name."
        return
    fi

    # Check if requirements are installed
    if [ ! -d "$dir/venv" ]; then
        echo "  Creating virtual environment..."
        cd $dir
        python3 -m venv venv
        source venv/bin/activate
        pip install -r requirements.txt
        deactivate
        cd - > /dev/null
    fi

    # Start service in background
    cd $dir
    source venv/bin/activate
    nohup python3 $script > ${name}.log 2>&1 &
    echo $! > ${name}.pid
    deactivate
    cd - > /dev/null

    echo "  Started $name (PID: $(cat $dir/${name}.pid))"
    echo "  Logs: $dir/${name}.log"
}

# Create logs directory
mkdir -p logs

# Start ML services
echo ""
echo "Starting ML Services..."
echo "------------------------"

# Check if regime detection is enabled
if grep -q "^REGIME_ML_SERVICE_URL=http://localhost:8001" .env 2>/dev/null; then
    start_service "regime-detector" "python/regime_detector" "regime_ml_service.py" 8001
else
    echo "Regime ML service disabled in .env (using rule-based detection)"
fi

# Check if news sentiment is enabled
if grep -q "^NEWS_SENTIMENT_SERVICE_URL=http://localhost:8002" .env 2>/dev/null; then
    start_service "news-sentiment" "python/news_sentiment" "finbert_service.py" 8002
else
    echo "News sentiment service disabled in .env (using keyword-based analysis)"
fi

# Wait for services to start
echo ""
echo "Waiting for ML services to start..."
sleep 5

# Health check
echo ""
echo "Health Check..."
echo "---------------"

if lsof -Pi :8001 -sTCP:LISTEN -t >/dev/null 2>&1; then
    if curl -s http://localhost:8001/health > /dev/null 2>&1; then
        echo "✓ Regime detector is healthy"
    else
        echo "✗ Regime detector is not responding"
    fi
else
    echo "- Regime detector not running"
fi

if lsof -Pi :8002 -sTCP:LISTEN -t >/dev/null 2>&1; then
    if curl -s http://localhost:8002/health > /dev/null 2>&1; then
        echo "✓ News sentiment analyzer is healthy"
    else
        echo "✗ News sentiment analyzer is not responding"
    fi
else
    echo "- News sentiment analyzer not running"
fi

# Build Rust code
echo ""
echo "Building Trading Agent..."
echo "-------------------------"
cargo build --release

# Start trading agent
echo ""
echo "Starting Trading Agent..."
echo "-------------------------"

# Source .env
export $(cat .env | grep -v '^#' | xargs)

# Run trading agent
echo ""
echo "=================================================="
echo "Trading Agent Starting..."
echo "=================================================="
echo ""
echo "Features enabled:"
grep -E "^(USE_KELLY_SIZING|ENABLE_MULTI_TIMEFRAME|ENABLE_REGIME_DETECTION|ENABLE_EXTENDED_HOURS|ENABLE_NEWS_TRADING)=" .env | sed 's/^/  /'
echo ""
echo "Press Ctrl+C to stop"
echo ""

cargo run --release --bin trading-agent
