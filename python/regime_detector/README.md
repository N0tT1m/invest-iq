# Market Regime Detection ML Service

This service provides machine learning-based market regime detection for the InvestIQ trading system.

## Features

- Classifies markets into 5 regimes:
  - **Trending Bullish**: Strong upward trend with low volatility
  - **Trending Bearish**: Strong downward trend with low volatility
  - **Ranging**: Sideways movement with clear support/resistance
  - **Volatile**: High volatility with rapid price swings
  - **Calm**: Low volatility, tight price range

- Uses multiple technical features:
  - Volatility (rolling standard deviation)
  - Trend strength (linear regression slope)
  - Average True Range (ATR)
  - Range efficiency
  - Momentum
  - Moving average convergence

## Installation

```bash
cd python/regime_detector
pip install -r requirements.txt
```

## Running the Service

```bash
# Start the service on port 8001
uvicorn regime_ml_service:app --host 0.0.0.0 --port 8001

# Or run directly
python regime_ml_service.py
```

## API Endpoints

### Health Check
```bash
curl http://localhost:8001/health
```

### Detect Regime
```bash
curl -X POST http://localhost:8001/detect_regime \
  -H "Content-Type: application/json" \
  -d '{
    "bars": [
      {
        "timestamp": 1699000000,
        "open": 100.0,
        "high": 101.0,
        "low": 99.0,
        "close": 100.5,
        "volume": 1000000.0
      }
    ]
  }'
```

## Integration with Rust

The Rust `market-regime-detector` crate automatically queries this service when configured:

```rust
use market_regime_detector::MarketRegimeDetector;

let detector = MarketRegimeDetector::with_ml_service(
    "http://localhost:8001".to_string()
);

let result = detector.detect_regime_ml(&bars).await?;
println!("Regime: {:?}", result.regime);
```

## Environment Configuration

Add to your `.env`:

```bash
# Optional: URL to ML regime detection service
REGIME_ML_SERVICE_URL=http://localhost:8001
```

## Future Enhancements

1. **Train ML Model**: Currently uses rule-based detection. Can be enhanced with:
   - Historical data training
   - Random Forest / XGBoost classifier
   - Deep learning (LSTM) for sequence modeling

2. **Additional Features**:
   - Volume profile analysis
   - Order flow imbalance
   - Market microstructure features

3. **Real-time Updates**:
   - WebSocket support for streaming regime updates
   - Redis caching for performance

## GPU Acceleration

This service can leverage your 5090/4090 GPUs for deep learning models:

```python
# Install PyTorch with CUDA support
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
```

Then use GPU-accelerated models for faster inference.
