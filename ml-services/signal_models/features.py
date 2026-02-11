"""Feature engineering: extract training features from DB and analysis results."""
import json
import sqlite3
import numpy as np
from typing import Dict, List, Optional, Any
from pathlib import Path
from contextlib import contextmanager


# 23-feature vector definition
FEATURE_NAMES = [
    # Engine signals (4)
    "technical_score", "fundamental_score", "quant_score", "sentiment_score",
    # Engine confidences (4)
    "technical_confidence", "fundamental_confidence", "quant_confidence", "sentiment_confidence",
    # Technical metrics (4)
    "rsi", "bb_percent_b", "adx", "sma_20_vs_50",
    # Fundamental metrics (4)
    "pe_ratio", "debt_to_equity", "revenue_growth", "roic",
    # Quant metrics (4)
    "sharpe_ratio", "volatility", "max_drawdown", "beta",
    # Sentiment metrics (3)
    "normalized_sentiment_score", "article_count", "direct_mention_ratio",
    # Market context (3) -- note: market_regime is encoded as float
    "market_regime_encoded", "inter_engine_agreement", "vix_proxy",
]

NUM_FEATURES = len(FEATURE_NAMES)  # 23


def encode_market_regime(regime: Optional[str]) -> float:
    """Encode market regime as numeric value."""
    mapping = {
        "low_volatility": -1.0,
        "normal": 0.0,
        "high_volatility": 1.0,
        "unknown": 0.0,
    }
    return mapping.get(regime or "unknown", 0.0)


def extract_features_from_analysis(analysis: Dict[str, Any]) -> Dict[str, float]:
    """Extract 23-feature vector from a UnifiedAnalysis JSON response.

    Args:
        analysis: The full analysis response dict (as returned by /api/analyze/:symbol)

    Returns:
        Dict mapping feature name to float value. Missing values default to 0.0.
    """
    features: Dict[str, float] = {}

    # Helper to safely get nested metric values
    def get_metric(engine_key: str, metric_name: str, default: float = 0.0) -> float:
        engine = analysis.get(engine_key)
        if not engine:
            return default
        metrics = engine.get("metrics", {})
        if isinstance(metrics, str):
            try:
                metrics = json.loads(metrics)
            except (json.JSONDecodeError, TypeError):
                return default
        val = metrics.get(metric_name, default)
        if val is None:
            return default
        return float(val)

    # Signal strength mapping
    signal_map = {
        "StrongBuy": 100, "Buy": 60, "WeakBuy": 30,
        "Neutral": 0,
        "WeakSell": -30, "Sell": -60, "StrongSell": -100,
    }

    def get_signal_score(engine_key: str) -> float:
        engine = analysis.get(engine_key)
        if not engine:
            return 0.0
        signal = engine.get("signal", "Neutral")
        return float(signal_map.get(signal, 0))

    def get_confidence(engine_key: str) -> float:
        engine = analysis.get(engine_key)
        if not engine:
            return 0.0
        return float(engine.get("confidence", 0.0))

    # Engine signals (4)
    features["technical_score"] = get_signal_score("technical")
    features["fundamental_score"] = get_signal_score("fundamental")
    features["quant_score"] = get_signal_score("quantitative")
    features["sentiment_score"] = get_signal_score("sentiment")

    # Engine confidences (4)
    features["technical_confidence"] = get_confidence("technical")
    features["fundamental_confidence"] = get_confidence("fundamental")
    features["quant_confidence"] = get_confidence("quantitative")
    features["sentiment_confidence"] = get_confidence("sentiment")

    # Technical metrics (4)
    features["rsi"] = get_metric("technical", "rsi", 50.0)
    features["bb_percent_b"] = get_metric("technical", "bb_percent_b", 0.5)
    features["adx"] = get_metric("technical", "adx", 20.0)
    sma_20 = get_metric("technical", "sma_20", 0.0)
    sma_50 = get_metric("technical", "sma_50", 0.0)
    features["sma_20_vs_50"] = 1.0 if sma_20 > sma_50 else (-1.0 if sma_50 > sma_20 else 0.0)

    # Fundamental metrics (4)
    features["pe_ratio"] = get_metric("fundamental", "pe_ratio", 20.0)
    features["debt_to_equity"] = get_metric("fundamental", "debt_to_equity", 1.0)
    features["revenue_growth"] = get_metric("fundamental", "revenue_growth", 0.0)
    features["roic"] = get_metric("fundamental", "roic", 0.0)

    # Quant metrics (4)
    features["sharpe_ratio"] = get_metric("quantitative", "sharpe_ratio", 0.0)
    features["volatility"] = get_metric("quantitative", "volatility", 0.2)
    features["max_drawdown"] = get_metric("quantitative", "max_drawdown", 0.0)
    features["beta"] = get_metric("quantitative", "beta", 1.0)

    # Sentiment metrics (3)
    features["normalized_sentiment_score"] = get_metric("sentiment", "normalized_score", 0.0)
    features["article_count"] = get_metric("sentiment", "article_count", 0.0)
    features["direct_mention_ratio"] = get_metric("sentiment", "direct_mention_ratio", 0.5)

    # Market context (3)
    regime = analysis.get("market_regime", "normal")
    features["market_regime_encoded"] = encode_market_regime(regime)

    # Inter-engine agreement: std dev of 4 engine scores (lower = more agreement)
    scores = [
        features["technical_score"],
        features["fundamental_score"],
        features["quant_score"],
        features["sentiment_score"],
    ]
    active_scores = [s for s in scores if s != 0.0]
    if len(active_scores) >= 2:
        features["inter_engine_agreement"] = float(np.std(active_scores))
    else:
        features["inter_engine_agreement"] = 0.0

    # VIX proxy: use volatility from quant engine as a stand-in
    features["vix_proxy"] = features["volatility"]

    return features


def features_to_array(features: Dict[str, float]) -> np.ndarray:
    """Convert feature dict to ordered numpy array matching FEATURE_NAMES."""
    return np.array([features.get(name, 0.0) for name in FEATURE_NAMES], dtype=np.float32)


def features_from_array(arr: np.ndarray) -> Dict[str, float]:
    """Convert ordered numpy array back to feature dict."""
    return {name: float(arr[i]) for i, name in enumerate(FEATURE_NAMES)}


@contextmanager
def get_db_connection(db_path: str):
    """Context manager for SQLite connections."""
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    try:
        yield conn
    finally:
        conn.close()


def load_analysis_features(db_path: str, min_samples: int = 0) -> List[Dict[str, Any]]:
    """Load analysis features from DB for training.

    Returns list of dicts with keys: features (dict), overall_signal, overall_confidence,
    actual_return_5d, actual_return_20d, symbol, analysis_date.
    """
    with get_db_connection(db_path) as conn:
        cursor = conn.cursor()
        cursor.execute("""
            SELECT symbol, analysis_date, features_json, overall_signal,
                   overall_confidence, actual_return_5d, actual_return_20d
            FROM analysis_features
            WHERE evaluated = 1 AND actual_return_5d IS NOT NULL
            ORDER BY analysis_date ASC
        """)
        rows = cursor.fetchall()

    results = []
    for row in rows:
        try:
            features = json.loads(row["features_json"])
            results.append({
                "features": features,
                "overall_signal": row["overall_signal"],
                "overall_confidence": row["overall_confidence"],
                "actual_return_5d": row["actual_return_5d"],
                "actual_return_20d": row["actual_return_20d"],
                "symbol": row["symbol"],
                "analysis_date": row["analysis_date"],
            })
        except (json.JSONDecodeError, TypeError):
            continue

    if len(results) < min_samples:
        return []
    return results


def load_backtest_trades(db_path: str) -> List[Dict[str, Any]]:
    """Load backtest trades from DB for training data.

    Returns list of dicts with: symbol, signal_type, confidence, entry_price,
    exit_price, pnl_pct, trade_date, features (if available).
    """
    with get_db_connection(db_path) as conn:
        cursor = conn.cursor()
        cursor.execute("""
            SELECT symbol, signal, confidence, entry_price, exit_price,
                   profit_loss_percent, entry_date
            FROM backtest_trades
            ORDER BY entry_date ASC
        """)
        rows = cursor.fetchall()

    results = []
    for row in rows:
        results.append({
            "symbol": row["symbol"],
            "signal_type": row["signal"],
            "confidence": row["confidence"],
            "entry_price": row["entry_price"],
            "exit_price": row["exit_price"],
            "pnl_pct": row["profit_loss_percent"],
            "trade_date": row["entry_date"],
        })

    return results
