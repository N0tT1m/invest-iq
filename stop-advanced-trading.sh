#!/bin/bash
# Stop all InvestIQ advanced trading services

set -e

echo "=================================================="
echo "Stopping InvestIQ Advanced Trading System"
echo "=================================================="
echo ""

# Function to stop a service
stop_service() {
    local name=$1
    local dir=$2

    if [ -f "$dir/${name}.pid" ]; then
        local pid=$(cat "$dir/${name}.pid")
        echo "Stopping $name (PID: $pid)..."
        if ps -p $pid > /dev/null 2>&1; then
            kill $pid
            echo "  Stopped $name"
        else
            echo "  $name was not running"
        fi
        rm -f "$dir/${name}.pid"
    else
        echo "$name PID file not found (may not be running)"
    fi
}

# Stop ML services
echo "Stopping ML Services..."
echo "-----------------------"

stop_service "regime-detector" "python/regime_detector"
stop_service "news-sentiment" "python/news_sentiment"

# Kill any remaining processes on those ports
echo ""
echo "Checking for remaining processes..."
echo "-----------------------------------"

for port in 8001 8002; do
    pid=$(lsof -ti:$port 2>/dev/null || true)
    if [ ! -z "$pid" ]; then
        echo "Killing process on port $port (PID: $pid)"
        kill -9 $pid 2>/dev/null || true
    fi
done

echo ""
echo "All services stopped"
