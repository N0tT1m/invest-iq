#!/bin/bash

# InvestIQ Dash Frontend Startup Script

echo "🚀 InvestIQ Dashboard Startup"
echo "=============================="

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is not installed. Please install Python 3.8 or higher."
    exit 1
fi

echo "✅ Python found: $(python3 --version)"

# Check if virtual environment exists
if [ ! -d "venv" ]; then
    echo "📦 Creating virtual environment..."
    python3 -m venv venv
fi

# Activate virtual environment
echo "🔧 Activating virtual environment..."
source venv/bin/activate

# Install/update requirements
echo "📥 Installing dependencies..."
pip install -q --upgrade pip
pip install -q -r requirements.txt

# Check if API server is running
echo "🔍 Checking API server..."
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "✅ API server is running"
else
    echo "⚠️  WARNING: API server not detected at http://localhost:3000"
    echo "   Please start the API server first:"
    echo "   cd .. && cargo run --release --bin api-server"
    echo ""
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Start the dashboard
echo ""
echo "🎨 Starting InvestIQ Dashboard..."
echo "📊 Dashboard will be available at: http://localhost:8050"
echo "⏹️  Press Ctrl+C to stop"
echo ""

python3 app.py
