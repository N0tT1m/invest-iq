#!/bin/bash

# InvestIQ - Docker Compose Startup Script
# Ensures API_KEY is properly set before starting services

set -e

echo "======================================"
echo "🐳 InvestIQ - Docker Compose Start"
echo "======================================"
echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found"
    echo "Please copy .env.example to .env and add your API keys:"
    echo "  cp .env.example .env"
    exit 1
fi

# Load environment variables
echo "Loading environment variables from .env..."
export $(grep -v '^#' .env | xargs)

# Extract first API key from API_KEYS and export as API_KEY
if [ -z "$API_KEY" ]; then
    if [ ! -z "$API_KEYS" ]; then
        export API_KEY=$(echo $API_KEYS | cut -d',' -f1)
        echo "✅ API_KEY extracted from API_KEYS"
    else
        echo "⚠️  Warning: Neither API_KEY nor API_KEYS set in .env"
        echo "API authentication may not work properly"
    fi
fi

# Check required keys
if [ -z "$POLYGON_API_KEY" ]; then
    echo "⚠️  Warning: POLYGON_API_KEY not set"
fi

echo ""
echo "Starting services with docker-compose..."
echo ""

# Start services
docker-compose up -d

echo ""
echo "======================================"
echo "✅ Services Started!"
echo "======================================"
echo ""
echo "📊 Dashboard:  http://localhost:8050"
echo "🔌 API Server: http://localhost:3000"
echo "💾 Redis:      localhost:6379"
echo ""
echo "View logs:"
echo "  docker-compose logs -f"
echo ""
echo "Stop services:"
echo "  docker-compose down"
echo ""
