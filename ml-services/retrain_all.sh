#!/bin/bash
# Retrain all ML models weekly
# Fetches all active US tickers from Polygon dynamically (requires POLYGON_API_KEY)

set -e

echo "========================================="
echo "InvestIQ ML Model Retraining Pipeline"
echo "========================================="
echo ""

# Activate virtual environment
source venv/bin/activate

# Build Rust data fetcher (20-50x faster Polygon fetching)
echo "0. Building Rust data fetcher..."
if command -v maturin &> /dev/null; then
    # VIRTUAL_ENV must be set so maturin installs into the correct venv
    (cd ../crates/invest-iq-data && VIRTUAL_ENV="$VIRTUAL_ENV" maturin develop --release) && echo "  Rust fetcher built successfully" || echo "  Rust fetcher build failed, falling back to Python"
else
    echo "  maturin not installed (pip install maturin), using Python fetcher"
fi
echo ""

# Create backup of current models
echo "1. Backing up current models..."
BACKUP_DIR="models/backups/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp -r models/sentiment/fine-tuned "$BACKUP_DIR/sentiment" 2>/dev/null || echo "  No sentiment model to backup"
cp -r models/price_predictor/trained "$BACKUP_DIR/price_predictor" 2>/dev/null || echo "  No price predictor model to backup"
echo "  Backup saved to: $BACKUP_DIR"
echo ""

# Pre-fetch all training data into DB (bars + news + analysis features)
echo "1.5. Pre-fetching all training data via data-loader..."
echo "  This populates training_bars and training_news tables"
echo ""
cargo run -p data-loader --release -- --all --all-data --db ../portfolio.db
echo "  Data pre-fetch complete"
echo ""

# Retrain FinBERT Sentiment (from pre-fetched DB data)
echo "2. Retraining FinBERT Sentiment Model..."
echo "  Using pre-fetched news from training_news table"
echo ""
python sentiment/train.py \
    --from-db \
    --db-path ../portfolio.db \
    --output-dir ./models/sentiment/fine-tuned \
    --epochs 3 \
    --batch-size 16

echo ""

# Retrain PatchTST Price Predictor (from pre-fetched DB data)
echo "3. Retraining PatchTST Price Predictor..."
echo "  Using pre-fetched bars from training_bars table"
echo ""
python price_predictor/train.py \
    --from-db \
    --db-path ../portfolio.db \
    --epochs 100 \
    --batch-size 64 \
    --learning-rate 1e-4 \
    --output-dir ./models/price_predictor/trained \
    --early-stopping 15

echo ""
echo "4. Updating Bayesian Strategy Weights..."
echo "  Syncing from last 7 days of trades..."
curl -X POST "http://localhost:8002/sync-from-database?days=7" || echo "  Bayesian service not running, skipping"
echo ""

echo "5. Generating signal training data (Polygon-direct if Rust fetcher available)..."
python signal_models/generate_data.py \
    --days 365 \
    --output ./data/signal_training_data.json
echo ""

echo "6. Retraining Signal Models (Meta-Model, Calibrator, Weight Optimizer)..."
python signal_models/train.py --db-path ../portfolio.db --output-dir ./models/signal_models
echo ""

echo "========================================="
echo "Retraining Complete!"
echo "========================================="
echo ""
echo "Model locations:"
echo "  - FinBERT:         models/sentiment/fine-tuned"
echo "  - Price Predictor: models/price_predictor/trained"
echo "  - Signal Models:   models/signal_models"
echo "  - Backups:         $BACKUP_DIR"
echo ""
echo "Restart ML services to use new models:"
echo "  ./stop_all_services.sh"
echo "  ./start_all_services.sh"
echo ""
