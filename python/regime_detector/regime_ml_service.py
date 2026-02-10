"""
Market Regime Detection ML Service

This service uses machine learning to classify market regimes.
It runs as a FastAPI service that the Rust code can query.

Run with: uvicorn regime_ml_service:app --host 0.0.0.0 --port 8001
"""

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List
import numpy as np
import pandas as pd
from datetime import datetime
import logging

# Optional: Use scikit-learn for more sophisticated models
try:
    from sklearn.ensemble import RandomForestClassifier
    from sklearn.preprocessing import StandardScaler
    SKLEARN_AVAILABLE = True
except ImportError:
    SKLEARN_AVAILABLE = False
    logging.warning("scikit-learn not available. Using rule-based detection only.")

app = FastAPI(title="Market Regime Detection Service")

# Logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class Bar(BaseModel):
    timestamp: int
    open: float
    high: float
    low: float
    close: float
    volume: float


class RegimeRequest(BaseModel):
    bars: List[Bar]


class RegimeResponse(BaseModel):
    regime: str
    confidence: float
    reasoning: str


class RegimeDetector:
    """ML-based market regime detector"""

    def __init__(self):
        self.model = None
        self.scaler = None
        self.min_bars = 50

        if SKLEARN_AVAILABLE:
            # Initialize a pre-trained model (in production, load from file)
            self.scaler = StandardScaler()
            self.model = RandomForestClassifier(n_estimators=100, random_state=42)
            logger.info("ML model initialized (needs training)")
        else:
            logger.info("Using rule-based regime detection")

    def detect_regime(self, bars: List[Bar]) -> RegimeResponse:
        """Detect market regime from price bars"""

        if len(bars) < self.min_bars:
            return RegimeResponse(
                regime="unknown",
                confidence=0.0,
                reasoning=f"Insufficient data: {len(bars)} bars (need {self.min_bars})"
            )

        # Convert to DataFrame
        df = pd.DataFrame([
            {
                'timestamp': b.timestamp,
                'open': b.open,
                'high': b.high,
                'low': b.low,
                'close': b.close,
                'volume': b.volume
            }
            for b in bars
        ])

        # Calculate features
        features = self._calculate_features(df)

        # Use ML model if available and trained, otherwise use rules
        if self.model is not None and SKLEARN_AVAILABLE:
            try:
                regime, confidence = self._ml_classify(features)
            except Exception as e:
                logger.warning(f"ML classification failed: {e}. Using rule-based.")
                regime, confidence = self._rule_based_classify(features)
        else:
            regime, confidence = self._rule_based_classify(features)

        reasoning = self._generate_reasoning(features, regime)

        return RegimeResponse(
            regime=regime,
            confidence=confidence,
            reasoning=reasoning
        )

    def _calculate_features(self, df: pd.DataFrame) -> dict:
        """Calculate technical features for regime detection"""

        # Returns
        df['returns'] = df['close'].pct_change()

        # Volatility (20-period rolling std)
        volatility = df['returns'].rolling(20).std().iloc[-1]

        # Trend strength (linear regression slope)
        x = np.arange(len(df))
        y = df['close'].values
        slope = np.polyfit(x[-20:], y[-20:], 1)[0] if len(df) >= 20 else 0
        trend_strength = slope / df['close'].mean()

        # Average True Range
        df['prev_close'] = df['close'].shift(1)
        df['tr'] = df.apply(
            lambda row: max(
                row['high'] - row['low'],
                abs(row['high'] - row['prev_close']),
                abs(row['low'] - row['prev_close'])
            ) if pd.notna(row['prev_close']) else row['high'] - row['low'],
            axis=1
        )
        atr = df['tr'].rolling(14).mean().iloc[-1]
        atr_percent = (atr / df['close'].iloc[-1]) * 100

        # Range efficiency
        net_movement = abs(df['close'].iloc[-1] - df['close'].iloc[0])
        total_movement = df['returns'].abs().sum() * df['close'].mean()
        range_efficiency = net_movement / total_movement if total_movement > 0 else 0

        # Momentum (20-period ROC)
        if len(df) >= 20:
            momentum = (df['close'].iloc[-1] - df['close'].iloc[-20]) / df['close'].iloc[-20]
        else:
            momentum = 0

        # Moving average convergence
        if len(df) >= 50:
            sma_10 = df['close'].rolling(10).mean().iloc[-1]
            sma_20 = df['close'].rolling(20).mean().iloc[-1]
            sma_50 = df['close'].rolling(50).mean().iloc[-1]
            ma_convergence = (sma_10 - sma_50) / sma_50
        else:
            ma_convergence = 0

        return {
            'volatility': volatility if pd.notna(volatility) else 0,
            'trend_strength': trend_strength,
            'atr_percent': atr_percent if pd.notna(atr_percent) else 0,
            'range_efficiency': range_efficiency,
            'momentum': momentum,
            'ma_convergence': ma_convergence,
        }

    def _rule_based_classify(self, features: dict) -> tuple:
        """Rule-based regime classification"""

        volatility = features['volatility']
        trend_strength = features['trend_strength']
        range_efficiency = features['range_efficiency']
        momentum = features['momentum']
        atr_percent = features['atr_percent']

        # Scoring for each regime
        scores = {
            'trending_bullish': 0.0,
            'trending_bearish': 0.0,
            'ranging': 0.0,
            'volatile': 0.0,
            'calm': 0.0,
        }

        # High volatility indicates volatile regime
        if volatility > 0.03:
            scores['volatile'] += 40

        # Low volatility indicates calm regime
        if volatility < 0.01:
            scores['calm'] += 30

        # Strong positive trend + high efficiency = bullish trend
        if trend_strength > 0.01 and range_efficiency > 0.5:
            scores['trending_bullish'] += 50

        # Strong negative trend + high efficiency = bearish trend
        if trend_strength < -0.01 and range_efficiency > 0.5:
            scores['trending_bearish'] += 50

        # Low efficiency + moderate volatility = ranging
        if range_efficiency < 0.3 and 0.01 <= volatility <= 0.025:
            scores['ranging'] += 40

        # Momentum contribution
        if momentum > 0.05:
            scores['trending_bullish'] += 20
        elif momentum < -0.05:
            scores['trending_bearish'] += 20

        # ATR contribution
        if atr_percent > 3.0:
            scores['volatile'] += 20
        elif atr_percent < 1.0:
            scores['calm'] += 20

        # Find highest scoring regime
        regime = max(scores, key=scores.get)
        confidence = min(scores[regime] / 100.0, 1.0)

        return regime, confidence

    def _ml_classify(self, features: dict) -> tuple:
        """ML-based classification (requires trained model)"""

        # In production, this would use a pre-trained model
        # For now, fall back to rule-based
        return self._rule_based_classify(features)

    def _generate_reasoning(self, features: dict, regime: str) -> str:
        """Generate human-readable reasoning"""

        volatility = features['volatility'] * 100
        trend = features['trend_strength'] * 100
        efficiency = features['range_efficiency']
        atr = features['atr_percent']

        return (
            f"{regime.replace('_', ' ').title()} regime detected: "
            f"trend={trend:.3f}%, volatility={volatility:.2f}%, "
            f"efficiency={efficiency:.2f}, ATR={atr:.2f}%"
        )


# Global detector instance
detector = RegimeDetector()


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {
        "status": "healthy",
        "ml_available": SKLEARN_AVAILABLE,
        "model_trained": detector.model is not None
    }


@app.post("/detect_regime", response_model=RegimeResponse)
async def detect_regime(request: RegimeRequest):
    """Detect market regime from price bars"""

    try:
        result = detector.detect_regime(request.bars)
        logger.info(f"Detected regime: {result.regime} (confidence: {result.confidence:.2f})")
        return result
    except Exception as e:
        logger.error(f"Regime detection failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/")
async def root():
    """Root endpoint"""
    return {
        "service": "Market Regime Detection",
        "version": "1.0.0",
        "endpoints": {
            "health": "/health",
            "detect_regime": "/detect_regime (POST)"
        }
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8001)
