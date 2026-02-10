#!/bin/bash
# Retrain all ML models weekly

set -e

echo "========================================="
echo "InvestIQ ML Model Retraining Pipeline"
echo "========================================="
echo ""

# Activate virtual environment
source venv/bin/activate

# Create backup of current models
echo "1. Backing up current models..."
BACKUP_DIR="models/backups/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp -r models/sentiment/trained "$BACKUP_DIR/sentiment" 2>/dev/null || echo "  No sentiment model to backup"
cp -r models/price_predictor/trained "$BACKUP_DIR/price_predictor" 2>/dev/null || echo "  No price predictor model to backup"
echo "  Backup saved to: $BACKUP_DIR"
echo ""

# Retrain FinBERT (optional, usually pre-trained is sufficient)
echo "2. FinBERT Sentiment Model"
echo "  Using pre-trained FinBERT (ProsusAI/finbert)"
echo "  To fine-tune on custom data, run:"
echo "    python sentiment/train.py --dataset your_data.csv"
echo ""

# Retrain PatchTST Price Predictor
echo "3. Retraining PatchTST Price Predictor..."
echo "  This will fetch 60 days of data and train for 50 epochs"
echo "  Estimated time: 30-60 minutes on GPU"
echo ""
python price_predictor/train.py \
    --symbols SPY QQQ AAPL MSFT GOOGL TSLA NVDA META AMZN \
    --days 60 \
    --interval 15m \
    --epochs 50 \
    --batch-size 64 \
    --learning-rate 1e-4 \
    --output-dir ./models/price_predictor/trained \
    --early-stopping 10

echo ""
echo "4. Updating Bayesian Strategy Weights..."
echo "  Syncing from last 7 days of trades..."
curl -X POST "http://localhost:8002/sync-from-database?days=7" || echo "  Bayesian service not running, skipping"
echo ""

echo "5. Retraining Signal Models (Meta-Model, Calibrator, Weight Optimizer)..."
python signal_models/train.py --db-path ../portfolio.db --output-dir ./models/signal_models
echo ""

echo "========================================="
echo "Retraining Complete!"
echo "========================================="
echo ""
echo "Model locations:"
echo "  - FinBERT:         models/sentiment/trained"
echo "  - Price Predictor: models/price_predictor/trained"
echo "  - Signal Models:   models/signal_models"
echo "  - Backups:         $BACKUP_DIR"
echo ""
echo "Restart ML services to use new models:"
echo "  ./stop_all_services.sh"
echo "  ./start_all_services.sh"
echo ""
