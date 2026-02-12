#!/bin/bash
# Start an individual ML service
# Usage: ./start_service.sh <service_name>
#   sentiment       - FinBERT Sentiment (port 8001)
#   bayesian        - Bayesian Strategy Weights (port 8002)
#   price_predictor - PatchTST Price Predictor (port 8003)
#   signal_models   - Signal Models (port 8004)

set -e

cd "$(dirname "$0")"

SERVICE="$1"

if [ -z "$SERVICE" ]; then
    echo "Usage: ./start_service.sh <service>"
    echo ""
    echo "Available services:"
    echo "  sentiment        FinBERT Sentiment          port 8001"
    echo "  bayesian         Bayesian Strategy Weights   port 8002"
    echo "  price_predictor  PatchTST Price Predictor    port 8003"
    echo "  signal_models    Signal Models                port 8004"
    exit 1
fi

# Map service name to module and port
case "$SERVICE" in
    sentiment)
        MODULE="sentiment.service"
        PORT=8001
        PID_FILE=".sentiment.pid"
        LABEL="FinBERT Sentiment"
        ;;
    bayesian)
        MODULE="bayesian.service"
        PORT=8002
        PID_FILE=".bayesian.pid"
        LABEL="Bayesian Strategy Weights"
        ;;
    price_predictor)
        MODULE="price_predictor.service"
        PORT=8003
        PID_FILE=".price.pid"
        LABEL="PatchTST Price Predictor"
        ;;
    signal_models)
        MODULE="signal_models.service"
        PORT=8004
        PID_FILE=".signal_models.pid"
        LABEL="Signal Models"
        ;;
    *)
        echo "Unknown service: $SERVICE"
        echo "Run ./start_service.sh with no args to see available services."
        exit 1
        ;;
esac

# Check if already running
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        echo "$LABEL is already running (PID: $OLD_PID)"
        echo "Stop it first: kill $OLD_PID"
        exit 1
    else
        rm "$PID_FILE"
    fi
fi

# Activate venv
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi
source venv/bin/activate

# Start in foreground or background
if [ "$2" = "--bg" ]; then
    echo "Starting $LABEL on port $PORT (background)..."
    python -m "$MODULE" &
    PID=$!
    echo "$PID" > "$PID_FILE"
    sleep 2
    if kill -0 "$PID" 2>/dev/null; then
        echo "$LABEL started (PID: $PID)"
        curl -s "http://localhost:$PORT/health" | python3 -m json.tool 2>/dev/null || echo "Health check pending..."
    else
        echo "Failed to start $LABEL"
        rm -f "$PID_FILE"
        exit 1
    fi
else
    echo "Starting $LABEL on port $PORT (foreground)..."
    echo "Press Ctrl+C to stop."
    echo ""
    python -m "$MODULE"
fi
