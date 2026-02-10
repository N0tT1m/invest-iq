#!/bin/bash
# Start all ML services

set -e

echo "Starting InvestIQ ML Services..."

# Check if virtual environment exists
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi

# Activate virtual environment
source venv/bin/activate

# Install dependencies
echo "Installing dependencies..."
pip install -r requirements.txt

# Start services in background
echo "Starting FinBERT Sentiment Service on port 8001..."
python -m sentiment.service &
SENTIMENT_PID=$!

sleep 2

echo "Starting Bayesian Strategy Weights Service on port 8002..."
python -m bayesian.service &
BAYESIAN_PID=$!

sleep 2

echo "Starting PatchTST Price Predictor Service on port 8003..."
python -m price_predictor.service &
PRICE_PID=$!

sleep 2

echo "Starting Signal Models Service on port 8004..."
python -m signal_models.service &
SIGNAL_PID=$!

sleep 2

# Save PIDs
echo $SENTIMENT_PID > .sentiment.pid
echo $BAYESIAN_PID > .bayesian.pid
echo $PRICE_PID > .price.pid
echo $SIGNAL_PID > .signal_models.pid

echo ""
echo "All ML services started!"
echo "  - FinBERT Sentiment:       http://localhost:8001"
echo "  - Bayesian Weights:        http://localhost:8002"
echo "  - Price Predictor:         http://localhost:8003"
echo "  - Signal Models:           http://localhost:8004"
echo ""
echo "PIDs saved to .*.pid files"
echo "To stop services, run: ./stop_all_services.sh"
echo ""
echo "Health checks:"
curl -s http://localhost:8001/health | jq '.' || echo "Sentiment service not ready yet"
curl -s http://localhost:8002/health | jq '.' || echo "Bayesian service not ready yet"
curl -s http://localhost:8003/health | jq '.' || echo "Price predictor service not ready yet"
curl -s http://localhost:8004/health | jq '.' || echo "Signal models service not ready yet"
