"""Feature engineering: extract training features from DB and analysis results."""
import json
import math
import sqlite3
import numpy as np
from typing import Dict, List, Optional, Any, Tuple
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

    # VIX proxy: prefer market-level volatility if available from analysis,
    # otherwise fall back to stock's own volatility as approximation.
    # The Rust API computes true VIX proxy from SPY bars.
    spy_vol = analysis.get("market_volatility") or analysis.get("spy_volatility")
    features["vix_proxy"] = float(spy_vol) if spy_vol is not None else features["volatility"]

    return features


def features_to_array(features: Dict[str, float]) -> np.ndarray:
    """Convert feature dict to ordered numpy array matching FEATURE_NAMES.

    Handles key mapping from Rust ML client (which uses shorter names) to the
    canonical training feature names.
    """
    # Map Rust client keys → training feature names
    ALIAS_MAP = {
        "tech_signal": "technical_score",
        "fund_signal": "fundamental_score",
        "quant_signal": "quant_score",
        "sent_signal": "sentiment_score",
        "tech_confidence": "technical_confidence",
        "fund_confidence": "fundamental_confidence",
        "quant_confidence": "quant_confidence",
        "sent_confidence": "sentiment_confidence",
        "rsi": "rsi",
        "macd_histogram": "bb_percent_b",       # best-effort: no bb_percent_b from Rust
        "bb_width": "sma_20_vs_50",             # best-effort mapping
        "adx": "adx",
        "pe_ratio": "pe_ratio",
        "debt_to_equity": "debt_to_equity",
        "sharpe_ratio": "sharpe_ratio",
        "var_95": "max_drawdown",               # closest proxy
        "max_drawdown": "max_drawdown",
        "volatility": "volatility",
        "news_sentiment_score": "normalized_sentiment_score",
        "regime": "market_regime_encoded",
        "spy_return": "inter_engine_agreement",  # repurposed slot
        "vix_proxy": "vix_proxy",
    }

    # Normalize keys: apply alias mapping
    normalized = {}
    for key, val in features.items():
        canonical = ALIAS_MAP.get(key, key)
        normalized[canonical] = val

    return np.array([normalized.get(name, 0.0) for name in FEATURE_NAMES], dtype=np.float32)


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

    Loads both evaluated rows (with real forward returns) and un-evaluated rows
    (using signal-derived pseudo-labels) so the model can bootstrap from dashboard
    analyses even before the data-loader backfills returns.
    """
    with get_db_connection(db_path) as conn:
        cursor = conn.cursor()
        cursor.execute("""
            SELECT symbol, analysis_date, features_json, overall_signal,
                   overall_confidence, actual_return_5d, actual_return_20d, evaluated
            FROM analysis_features
            ORDER BY analysis_date ASC
        """)
        rows = cursor.fetchall()

    signal_return_map = {
        "StrongBuy": 3.0, "Buy": 1.5, "WeakBuy": 0.5,
        "Neutral": 0.0,
        "WeakSell": -0.5, "Sell": -1.5, "StrongSell": -3.0,
    }

    results = []
    for row in rows:
        try:
            features = json.loads(row["features_json"])
            if not isinstance(features, dict):
                continue

            ret_5d = row["actual_return_5d"]
            if ret_5d is not None and abs(ret_5d) > 50:
                continue  # Skip extreme outliers

            # Use real return if available, otherwise derive from signal
            if ret_5d is None:
                signal = row["overall_signal"] or "Neutral"
                # Strip Rust debug formatting if present (e.g. "Buy" vs "SignalStrength::Buy")
                for key in signal_return_map:
                    if key in signal:
                        ret_5d = signal_return_map[key]
                        break
                else:
                    ret_5d = 0.0

            results.append({
                "features": features,
                "overall_signal": row["overall_signal"],
                "overall_confidence": row["overall_confidence"],
                "actual_return_5d": ret_5d,
                "actual_return_20d": row["actual_return_20d"],
                "symbol": row["symbol"],
                "analysis_date": row["analysis_date"],
            })
        except (json.JSONDecodeError, TypeError, KeyError):
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
        # Check if confidence column exists (added in migration 20240111)
        cursor.execute("PRAGMA table_info(backtest_trades)")
        columns = {col["name"] for col in cursor.fetchall()}
        has_confidence = "confidence" in columns

        if has_confidence:
            cursor.execute("""
                SELECT symbol, signal, COALESCE(confidence, 0.0) as confidence,
                       entry_price, exit_price,
                       profit_loss_percent, entry_date
                FROM backtest_trades
                ORDER BY entry_date ASC
            """)
        else:
            cursor.execute("""
                SELECT symbol, signal, entry_price, exit_price,
                       profit_loss_percent, entry_date
                FROM backtest_trades
                ORDER BY entry_date ASC
            """)
        rows = cursor.fetchall()

    results = []
    for row in rows:
        entry = row["entry_price"]
        exit_ = row["exit_price"]
        pnl = row["profit_loss_percent"]
        # Skip invalid trades: missing prices, extreme P/L
        if entry is None or exit_ is None or entry <= 0 or exit_ <= 0:
            continue
        if pnl is not None and abs(pnl) > 100:
            continue
        confidence = (row["confidence"] or 0.0) if has_confidence else 0.0
        results.append({
            "symbol": row["symbol"],
            "signal_type": row["signal"],
            "confidence": confidence,
            "entry_price": entry,
            "exit_price": exit_,
            "pnl_pct": pnl,
            "trade_date": row["entry_date"],
        })

    return results


# ---------------------------------------------------------------------------
# Generate training data from training_bars (populated by data-loader)
# ---------------------------------------------------------------------------

def _sma(closes: List[float], period: int) -> float:
    if len(closes) < period:
        return 0.0
    return sum(closes[-period:]) / period


def _rsi(closes: List[float], period: int = 14) -> float:
    if len(closes) < period + 1:
        return 50.0
    gains, losses = [], []
    for i in range(len(closes) - period, len(closes)):
        d = closes[i] - closes[i - 1]
        gains.append(max(d, 0.0))
        losses.append(max(-d, 0.0))
    avg_gain = sum(gains) / period
    avg_loss = sum(losses) / period
    if avg_loss == 0:
        return 100.0
    rs = avg_gain / avg_loss
    return 100.0 - (100.0 / (1.0 + rs))


def _volatility(closes: List[float], period: int = 20) -> float:
    if len(closes) < period + 1:
        return 0.0
    rets = []
    for i in range(len(closes) - period, len(closes)):
        if closes[i - 1] > 0:
            rets.append((closes[i] - closes[i - 1]) / closes[i - 1])
    if not rets:
        return 0.0
    mean_r = sum(rets) / len(rets)
    var = sum((r - mean_r) ** 2 for r in rets) / len(rets)
    return math.sqrt(var) * math.sqrt(252)


def _adx(highs: List[float], lows: List[float], closes: List[float], period: int = 14) -> float:
    """Simplified ADX approximation."""
    n = len(closes)
    if n < period + 1:
        return 20.0
    plus_dm, minus_dm, tr_list = [], [], []
    for i in range(n - period, n):
        h_diff = highs[i] - highs[i - 1]
        l_diff = lows[i - 1] - lows[i]
        plus_dm.append(max(h_diff, 0.0) if h_diff > l_diff else 0.0)
        minus_dm.append(max(l_diff, 0.0) if l_diff > h_diff else 0.0)
        tr = max(highs[i] - lows[i], abs(highs[i] - closes[i - 1]), abs(lows[i] - closes[i - 1]))
        tr_list.append(tr)
    atr = sum(tr_list) / period if tr_list else 1.0
    if atr == 0:
        return 20.0
    plus_di = (sum(plus_dm) / period) / atr * 100
    minus_di = (sum(minus_dm) / period) / atr * 100
    di_sum = plus_di + minus_di
    if di_sum == 0:
        return 20.0
    dx = abs(plus_di - minus_di) / di_sum * 100
    return dx


def _bb_percent_b(closes: List[float], period: int = 20) -> float:
    if len(closes) < period:
        return 0.5
    window = closes[-period:]
    sma = sum(window) / period
    std = math.sqrt(sum((c - sma) ** 2 for c in window) / period)
    if std == 0:
        return 0.5
    upper = sma + 2 * std
    lower = sma - 2 * std
    band_width = upper - lower
    if band_width == 0:
        return 0.5
    return (closes[-1] - lower) / band_width


def _beta(returns: List[float], benchmark_returns: List[float]) -> float:
    """Compute beta against benchmark returns."""
    n = min(len(returns), len(benchmark_returns))
    if n < 10:
        return 1.0
    r = returns[-n:]
    b = benchmark_returns[-n:]
    mean_r = sum(r) / n
    mean_b = sum(b) / n
    cov = sum((r[i] - mean_r) * (b[i] - mean_b) for i in range(n)) / n
    var_b = sum((b[i] - mean_b) ** 2 for i in range(n)) / n
    if var_b == 0:
        return 1.0
    return cov / var_b


def load_training_from_bars(
    db_path: str,
    forward_days: int = 5,
    min_bars: int = 60,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Generate training data from the training_bars table.

    For each symbol with enough bars, compute technical indicators at each
    point-in-time window and use forward returns as labels.

    Returns:
        X: feature matrix (N, NUM_FEATURES)
        y_cls: binary labels (1 = profitable)
        y_reg: return percentages
    """
    import logging
    logger = logging.getLogger(__name__)

    with get_db_connection(db_path) as conn:
        cursor = conn.cursor()
        # Check table exists
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='training_bars'")
        if not cursor.fetchone():
            logger.info("training_bars table not found")
            return np.empty((0, NUM_FEATURES)), np.empty(0), np.empty(0)

        # Get distinct symbols
        cursor.execute("SELECT DISTINCT symbol FROM training_bars WHERE timespan='day' ORDER BY symbol")
        symbols = [row["symbol"] for row in cursor.fetchall()]
        logger.info("Found %d symbols in training_bars", len(symbols))

        if not symbols:
            return np.empty((0, NUM_FEATURES)), np.empty(0), np.empty(0)

        # Load SPY bars for beta/benchmark (if available)
        cursor.execute("""
            SELECT close FROM training_bars
            WHERE symbol='SPY' AND timespan='day'
            ORDER BY timestamp_ms ASC
        """)
        spy_closes = [row["close"] for row in cursor.fetchall()]
        spy_returns = []
        for i in range(1, len(spy_closes)):
            if spy_closes[i - 1] > 0:
                spy_returns.append((spy_closes[i] - spy_closes[i - 1]) / spy_closes[i - 1])

    X_list = []
    y_cls_list = []
    y_reg_list = []

    for sym_idx, symbol in enumerate(symbols):
        if symbol == "SPY":
            continue  # Skip benchmark itself

        with get_db_connection(db_path) as conn:
            cursor = conn.cursor()
            cursor.execute("""
                SELECT open, high, low, close, volume
                FROM training_bars
                WHERE symbol=? AND timespan='day'
                ORDER BY timestamp_ms ASC
            """, (symbol,))
            rows = cursor.fetchall()

        if len(rows) < min_bars + forward_days:
            continue

        opens = [r["open"] for r in rows]
        highs = [r["high"] for r in rows]
        lows = [r["low"] for r in rows]
        closes = [r["close"] for r in rows]
        volumes = [r["volume"] for r in rows]

        # Slide a window: at each position t (from min_bars to len-forward_days),
        # compute features using bars[:t] and label using bars[t:t+forward_days]
        # Sample every 5 bars to keep dataset manageable
        step = 5
        for t in range(min_bars, len(closes) - forward_days, step):
            c = closes[:t]
            h = highs[:t]
            l = lows[:t]
            v = volumes[:t]
            current_price = c[-1]
            if current_price <= 0:
                continue

            future_price = closes[t + forward_days - 1]
            fwd_return = (future_price - current_price) / current_price * 100.0

            # Skip extreme outliers (likely data errors / splits)
            if abs(fwd_return) > 50:
                continue

            # Technical indicators
            sma_20 = _sma(c, 20)
            sma_50 = _sma(c, 50)
            rsi = _rsi(c, 14)
            vol = _volatility(c, 20)
            adx = _adx(h, l, c, 14)
            bb_pct = _bb_percent_b(c, 20)

            # Derive signal
            if sma_20 > sma_50 and rsi < 70:
                tech_signal = 60.0 if rsi < 60 else 30.0
            elif sma_20 < sma_50 and rsi > 30:
                tech_signal = -60.0 if rsi > 40 else -30.0
            else:
                tech_signal = 0.0

            tech_conf = 0.5
            if abs(tech_signal) >= 60:
                tech_conf = 0.7
            elif abs(tech_signal) >= 30:
                tech_conf = 0.55

            # Quant features from returns
            rets = []
            for i in range(max(1, len(c) - 60), len(c)):
                if c[i - 1] > 0:
                    rets.append((c[i] - c[i - 1]) / c[i - 1])
            sharpe = 0.0
            max_dd = 0.0
            if len(rets) >= 10:
                mean_r = sum(rets) / len(rets)
                std_r = math.sqrt(sum((r - mean_r) ** 2 for r in rets) / len(rets))
                if std_r > 0:
                    sharpe = (mean_r / std_r) * math.sqrt(252)
                # Max drawdown
                peak = c[0]
                for p in c:
                    if p > peak:
                        peak = p
                    dd = (peak - p) / peak if peak > 0 else 0
                    if dd > max_dd:
                        max_dd = dd

            beta_val = _beta(rets, spy_returns) if spy_returns else 1.0

            sma_20_vs_50 = 1.0 if sma_20 > sma_50 else (-1.0 if sma_50 > sma_20 else 0.0)

            # Volume-based sentiment proxy
            recent_vol = sum(v[-5:]) / 5 if len(v) >= 5 else 0
            avg_vol = sum(v[-20:]) / 20 if len(v) >= 20 else recent_vol
            vol_ratio = recent_vol / avg_vol if avg_vol > 0 else 1.0

            features = np.zeros(NUM_FEATURES, dtype=np.float32)
            features[0] = tech_signal           # technical_score
            features[1] = 0.0                   # fundamental_score (N/A from bars)
            features[2] = 0.0                   # quant_score (derived below)
            features[3] = 0.0                   # sentiment_score
            features[4] = tech_conf             # technical_confidence
            features[5] = 0.0                   # fundamental_confidence
            features[6] = 0.5                   # quant_confidence
            features[7] = 0.0                   # sentiment_confidence
            features[8] = rsi                   # rsi
            features[9] = bb_pct                # bb_percent_b
            features[10] = adx                  # adx
            features[11] = sma_20_vs_50         # sma_20_vs_50
            features[12] = 0.0                  # pe_ratio (N/A)
            features[13] = 0.0                  # debt_to_equity (N/A)
            features[14] = 0.0                  # revenue_growth (N/A)
            features[15] = 0.0                  # roic (N/A)
            features[16] = sharpe               # sharpe_ratio
            features[17] = vol                  # volatility
            features[18] = max_dd               # max_drawdown
            features[19] = beta_val             # beta
            features[20] = 0.0                  # normalized_sentiment_score
            features[21] = 0.0                  # article_count
            features[22] = vol_ratio            # direct_mention_ratio (repurpose as vol ratio)

            # Quant score: momentum signal
            if sharpe > 0.5:
                features[2] = 30.0
                features[6] = 0.6
            elif sharpe < -0.5:
                features[2] = -30.0
                features[6] = 0.6

            X_list.append(features)
            y_cls_list.append(1.0 if fwd_return > 0 else 0.0)
            y_reg_list.append(fwd_return)

        if (sym_idx + 1) % 500 == 0:
            logger.info("Processed %d/%d symbols (%d samples so far)",
                       sym_idx + 1, len(symbols), len(X_list))

    if not X_list:
        return np.empty((0, NUM_FEATURES)), np.empty(0), np.empty(0)

    logger.info("Generated %d training samples from training_bars (%d symbols)",
               len(X_list), len(symbols))

    return (
        np.array(X_list, dtype=np.float32),
        np.array(y_cls_list, dtype=np.float32),
        np.array(y_reg_list, dtype=np.float32),
    )
