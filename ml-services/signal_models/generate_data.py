"""Bootstrap training data from the backtester API or directly from Polygon.

Two modes:
  1. API mode (default): Calls local backtester API for each symbol (requires server running)
  2. Polygon-direct mode (--polygon-direct): Fetches bars + price changes from Polygon
     concurrently via the Rust invest_iq_data module (~20-50x faster, no server needed)

By default, fetches all active US stock tickers from Polygon for maximum coverage.
Use --symbols to override with a specific list.
"""
import argparse
import json
import logging
import math
import os
import sys
import time
import requests
from pathlib import Path
from typing import Dict, List, Any

sys.path.append(str(Path(__file__).parent.parent))

logger = logging.getLogger(__name__)

API_BASE = "http://localhost:3000"

try:
    import invest_iq_data
    _USE_RUST_FETCHER = True
except ImportError:
    _USE_RUST_FETCHER = False


def fetch_backtest(symbol: str, days: int = 365, timeout: int = 90) -> Dict[str, Any]:
    """Fetch backtest results for a symbol from the API."""
    try:
        resp = requests.get(
            f"{API_BASE}/api/backtest/{symbol}",
            params={"days": days},
            timeout=timeout,
        )
        resp.raise_for_status()
        return resp.json()
    except requests.RequestException as e:
        logger.warning("Failed to fetch backtest for %s: %s", symbol, e)
        return {}


def fetch_analysis(symbol: str, timeout: int = 90) -> Dict[str, Any]:
    """Fetch full analysis for a symbol to get feature vectors."""
    try:
        resp = requests.get(
            f"{API_BASE}/api/analyze/{symbol}",
            timeout=timeout,
        )
        resp.raise_for_status()
        return resp.json()
    except requests.RequestException as e:
        logger.warning("Failed to fetch analysis for %s: %s", symbol, e)
        return {}


def _compute_sma(closes: List[float], period: int) -> float:
    """Compute simple moving average from close prices."""
    if len(closes) < period:
        return 0.0
    return sum(closes[-period:]) / period


def _compute_rsi(closes: List[float], period: int = 14) -> float:
    """Compute RSI from close prices."""
    if len(closes) < period + 1:
        return 50.0
    gains = []
    losses = []
    for i in range(len(closes) - period, len(closes)):
        delta = closes[i] - closes[i - 1]
        if delta > 0:
            gains.append(delta)
            losses.append(0.0)
        else:
            gains.append(0.0)
            losses.append(abs(delta))
    avg_gain = sum(gains) / period
    avg_loss = sum(losses) / period
    if avg_loss == 0:
        return 100.0
    rs = avg_gain / avg_loss
    return 100.0 - (100.0 / (1.0 + rs))


def _compute_volatility(closes: List[float], period: int = 20) -> float:
    """Compute annualized volatility from close prices."""
    if len(closes) < period + 1:
        return 0.0
    returns = []
    for i in range(len(closes) - period, len(closes)):
        if closes[i - 1] > 0:
            returns.append((closes[i] - closes[i - 1]) / closes[i - 1])
    if not returns:
        return 0.0
    mean_r = sum(returns) / len(returns)
    variance = sum((r - mean_r) ** 2 for r in returns) / len(returns)
    daily_vol = math.sqrt(variance)
    return daily_vol * math.sqrt(252)


def _derive_signal(closes: List[float]) -> str:
    """Derive a basic signal from SMA crossover + RSI."""
    sma_20 = _compute_sma(closes, 20)
    sma_50 = _compute_sma(closes, 50)
    rsi = _compute_rsi(closes)

    if sma_20 == 0 or sma_50 == 0:
        return "Neutral"

    if sma_20 > sma_50 and rsi < 70:
        return "Buy" if rsi < 60 else "WeakBuy"
    elif sma_20 < sma_50 and rsi > 30:
        return "Sell" if rsi > 40 else "WeakSell"
    return "Neutral"


def generate_training_data_polygon(
    symbols: List[str],
    days: int = 365,
    output_path: str = "./data/signal_training_data.json",
    api_key: str = "",
    forward_days: int = 5,
) -> int:
    """Generate training data directly from Polygon using Rust concurrent fetcher.

    Fetches bars + price changes in bulk (no API server needed).
    Derives basic technical signals from bars and uses actual price changes as labels.
    """
    Path(output_path).parent.mkdir(parents=True, exist_ok=True)

    logger.info("Fetching bars for %d symbols (%dd lookback) via Rust...", len(symbols), days)
    all_bars = invest_iq_data.fetch_bars_multi(api_key, symbols, days, "day")
    logger.info("Got bars for %d symbols", len(all_bars))

    symbols_with_bars = list(all_bars.keys())
    logger.info("Fetching %d-day price changes for %d symbols via Rust...", forward_days, len(symbols_with_bars))
    price_changes = invest_iq_data.fetch_price_changes(api_key, symbols_with_bars, forward_days)
    logger.info("Got price changes for %d symbols", len(price_changes))

    all_samples = []

    for symbol, bars in all_bars.items():
        if len(bars) < 60:
            continue

        closes = [b["close"] for b in bars if b.get("close") is not None]
        if len(closes) < 60:
            continue

        # Derive signal from technical indicators
        signal = _derive_signal(closes)
        rsi = _compute_rsi(closes)
        sma_20 = _compute_sma(closes, 20)
        sma_50 = _compute_sma(closes, 50)
        volatility = _compute_volatility(closes)

        # Confidence based on indicator alignment
        confidence = 0.5
        if signal in ("Buy", "Sell"):
            confidence = 0.7
        elif signal in ("WeakBuy", "WeakSell"):
            confidence = 0.55

        # Use actual price change as outcome label
        pct_change = price_changes.get(symbol, 0.0)
        current_price = closes[-1] if closes else 0
        future_price = current_price * (1 + pct_change / 100) if current_price > 0 else 0

        last_bar = bars[-1]
        sample = {
            "symbol": symbol,
            "signal_type": signal,
            "confidence": confidence,
            "entry_price": current_price,
            "exit_price": future_price,
            "pnl_pct": pct_change,
            "trade_date": str(last_bar.get("timestamp", "")),
            "profitable": 1 if pct_change > 0 else 0,
            "analysis_features": {
                "technical": {
                    "signal": signal,
                    "confidence": confidence,
                    "metrics": {
                        "rsi": rsi,
                        "sma_20": sma_20,
                        "sma_50": sma_50,
                        "adx": 20.0,
                        "bb_percent_b": 0.5,
                    },
                },
                "quantitative": {
                    "signal": "Neutral",
                    "confidence": 0.5,
                    "metrics": {
                        "sharpe_ratio": 0.0,
                        "volatility": volatility,
                        "max_drawdown": 0.0,
                        "beta": 1.0,
                    },
                },
            },
        }
        all_samples.append(sample)

    with open(output_path, "w") as f:
        json.dump(all_samples, f)

    logger.info("Generated %d training samples, saved to %s", len(all_samples), output_path)
    return len(all_samples)


def generate_training_data(
    symbols: List[str],
    days: int = 365,
    output_path: str = "./data/signal_training_data.json",
    delay: float = 2.0,
) -> int:
    """Generate training data by calling backtest API for each symbol.

    Args:
        symbols: List of ticker symbols
        days: Backtest lookback days
        output_path: Where to save training data
        delay: Delay between API calls (rate limiting)

    Returns:
        Number of training samples generated
    """
    Path(output_path).parent.mkdir(parents=True, exist_ok=True)

    all_samples = []
    total_symbols = len(symbols)

    for i, symbol in enumerate(symbols):
        logger.info("[%d/%d] Processing %s...", i + 1, total_symbols, symbol)

        # Fetch backtest trades
        backtest = fetch_backtest(symbol, days)
        if not backtest:
            time.sleep(delay)
            continue

        trades = backtest.get("trades", [])
        if not trades:
            logger.info("  No trades for %s", symbol)
            time.sleep(delay)
            continue

        # Fetch analysis for feature vector
        analysis = fetch_analysis(symbol)
        time.sleep(delay)

        for trade in trades:
            pnl = trade.get("profit_loss_percent", 0) or 0
            sample = {
                "symbol": symbol,
                "signal_type": trade.get("signal", "Neutral"),
                "confidence": trade.get("confidence", 0.5),
                "entry_price": trade.get("entry_price", 0),
                "exit_price": trade.get("exit_price", 0),
                "pnl_pct": pnl,
                "trade_date": trade.get("entry_date", ""),
                "profitable": 1 if pnl > 0 else 0,
                "analysis_features": analysis if analysis else None,
            }
            all_samples.append(sample)

        logger.info("  %d trades collected for %s", len(trades), symbol)

    # Save as JSON (could also use parquet if pyarrow available)
    with open(output_path, "w") as f:
        json.dump(all_samples, f)

    logger.info("Generated %d training samples, saved to %s", len(all_samples), output_path)
    return len(all_samples)


def main():
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")

    parser = argparse.ArgumentParser(description="Generate training data from backtester")
    parser.add_argument("--symbols", nargs="+", default=None,
                       help="Symbols to backtest (default: all active tickers from Polygon)")
    parser.add_argument("--days", type=int, default=365, help="Backtest lookback days")
    parser.add_argument("--output", default="./data/signal_training_data.json", help="Output file")
    parser.add_argument("--delay", type=float, default=0.1, help="Delay between API calls (seconds)")
    parser.add_argument("--api-base", default=API_BASE, help="API base URL")
    parser.add_argument("--polygon-direct", action="store_true",
                       help="Fetch data directly from Polygon via Rust (no API server needed, much faster)")
    parser.add_argument("--forward-days", type=int, default=5,
                       help="Days forward for price change label (polygon-direct mode)")
    args = parser.parse_args()

    global API_BASE
    API_BASE = args.api_base

    # Resolve symbols: explicit list > dynamic fetch from Polygon
    if args.symbols:
        symbols = args.symbols
    else:
        from dotenv import load_dotenv
        load_dotenv()
        load_dotenv(dotenv_path="../.env")
        from shared.polygon import fetch_active_tickers
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if api_key:
            logger.info("Fetching all active tickers from Polygon...")
            symbols = fetch_active_tickers(api_key=api_key)
        else:
            logger.error("No --symbols provided and no POLYGON_API_KEY for dynamic fetch")
            sys.exit(1)

    logger.info("Training data generation for %d symbols", len(symbols))

    if args.polygon_direct:
        if not _USE_RUST_FETCHER:
            logger.error("--polygon-direct requires invest_iq_data Rust module. "
                        "Run: cd crates/invest-iq-data && maturin develop --release")
            sys.exit(1)
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if not api_key:
            logger.error("--polygon-direct requires POLYGON_API_KEY")
            sys.exit(1)
        n = generate_training_data_polygon(
            symbols, args.days, args.output, api_key, args.forward_days,
        )
    else:
        # Auto-upgrade to polygon-direct if Rust fetcher is available
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if _USE_RUST_FETCHER and api_key:
            logger.info("Rust fetcher available, using polygon-direct mode automatically")
            n = generate_training_data_polygon(
                symbols, args.days, args.output, api_key, args.forward_days,
            )
        else:
            n = generate_training_data(symbols, args.days, args.output, args.delay)

    print(f"\nDone! Generated {n} training samples.")


if __name__ == "__main__":
    main()
