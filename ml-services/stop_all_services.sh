#!/bin/bash
# Stop all ML services

echo "Stopping InvestIQ ML Services..."

# Stop sentiment service
if [ -f .sentiment.pid ]; then
    PID=$(cat .sentiment.pid)
    echo "Stopping FinBERT Sentiment Service (PID: $PID)..."
    kill $PID 2>/dev/null || echo "  Already stopped"
    rm .sentiment.pid
fi

# Stop bayesian service
if [ -f .bayesian.pid ]; then
    PID=$(cat .bayesian.pid)
    echo "Stopping Bayesian Weights Service (PID: $PID)..."
    kill $PID 2>/dev/null || echo "  Already stopped"
    rm .bayesian.pid
fi

# Stop price predictor service
if [ -f .price.pid ]; then
    PID=$(cat .price.pid)
    echo "Stopping Price Predictor Service (PID: $PID)..."
    kill $PID 2>/dev/null || echo "  Already stopped"
    rm .price.pid
fi

# Stop signal models service
if [ -f .signal_models.pid ]; then
    PID=$(cat .signal_models.pid)
    echo "Stopping Signal Models Service (PID: $PID)..."
    kill $PID 2>/dev/null || echo "  Already stopped"
    rm .signal_models.pid
fi

echo "All ML services stopped!"
