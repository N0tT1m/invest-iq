# ML Services Quick Start

Get ML enhancements running in 10 minutes.

## 1. Setup (5 min)

```bash
cd /Users/timmy/workspace/public-projects/invest-iq/ml-services

# Create environment
python3 -m venv venv
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

## 2. Train Price Predictor (30-60 min)

```bash
# This is REQUIRED - trains the price direction model
python price_predictor/train.py \
    --symbols SPY QQQ AAPL \
    --days 60 \
    --epochs 50 \
    --batch-size 64
```

Wait for training to complete. You'll see progress bars and final metrics.

## 3. Start Services (1 min)

```bash
./start_all_services.sh
```

Services running on:
- Sentiment: http://localhost:8001
- Bayesian: http://localhost:8002
- Price: http://localhost:8003

## 4. Test (1 min)

```bash
# Test sentiment
curl -X POST http://localhost:8001/predict \
  -H "Content-Type: application/json" \
  -d '{"texts": ["Stock market rallies on positive earnings"], "symbol": "SPY"}'

# Test Bayesian
curl http://localhost:8002/weights

# Test price predictor
curl http://localhost:8003/health
```

## 5. Update Trading Agent (2 min)

Add to `/Users/timmy/workspace/public-projects/invest-iq/.env`:

```bash
ML_SENTIMENT_URL=http://localhost:8001
ML_BAYESIAN_URL=http://localhost:8002
ML_PRICE_PREDICTOR_URL=http://localhost:8003
```

In `crates/trading-agent/src/main.rs`, replace:

```rust
use strategy_manager::StrategyManager;
let strategy_manager = StrategyManager::new(config.clone())?;
```

With:

```rust
mod ml_strategy_manager;
use ml_strategy_manager::MLStrategyManager;
let strategy_manager = MLStrategyManager::new(config.clone())?;
```

## 6. Run Trading Agent

```bash
cd /Users/timmy/workspace/public-projects/invest-iq
cargo build --release -p trading-agent
./target/release/trading-agent
```

## Done!

Your trading agent now uses:
- FinBERT for sentiment analysis
- Bayesian weights for strategy selection
- PatchTST for price prediction

## Weekly Maintenance

```bash
# Retrain models on fresh data
./retrain_all.sh

# Restart services
./stop_all_services.sh
./start_all_services.sh
```

## Troubleshooting

**Services won't start?**
```bash
# Check Python version (need 3.10+)
python3 --version

# Reinstall dependencies
pip install -r requirements.txt --force-reinstall
```

**Price predictor not loaded?**
```bash
# Train it first
python price_predictor/train.py --days 60 --epochs 10

# Verify files exist
ls models/price_predictor/trained/
# Should see: config.json, model.pt, normalization_stats.json
```

**GPU not detected?**
```bash
# Check CUDA
nvidia-smi

# Verify PyTorch sees GPU
python -c "import torch; print(torch.cuda.is_available())"

# Reinstall PyTorch with CUDA
pip install torch --index-url https://download.pytorch.org/whl/cu121
```

## Need Help?

See full documentation: `README.md` and `../ML_DEPLOYMENT_GUIDE.md`
