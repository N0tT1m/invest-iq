"""Bootstrap training data from the backtester API across multiple symbols."""
import argparse
import json
import logging
import time
import requests
from pathlib import Path
from typing import Dict, List, Any

logger = logging.getLogger(__name__)

# ~60 symbols covering all 11 GICS sectors
DEFAULT_SYMBOLS = [
    # Technology
    "AAPL", "MSFT", "GOOGL", "NVDA", "META", "AVGO",
    # Healthcare
    "JNJ", "UNH", "PFE", "ABBV", "MRK", "LLY",
    # Financials
    "JPM", "BAC", "GS", "V", "MA",
    # Energy
    "XOM", "CVX", "COP", "SLB", "EOG",
    # Consumer Discretionary
    "AMZN", "TSLA", "HD", "NKE", "SBUX",
    # Industrials
    "CAT", "BA", "HON", "UPS", "GE",
    # Utilities
    "NEE", "DUK", "SO", "AEP", "D",
    # Materials
    "LIN", "APD", "ECL", "SHW", "NEM",
    # Real Estate
    "AMT", "PLD", "CCI", "EQIX", "SPG",
    # Communications
    "NFLX", "DIS", "CMCSA", "T", "VZ",
    # Consumer Staples
    "PG", "KO", "PEP", "COST", "WMT",
]

API_BASE = "http://localhost:3000"


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
            sample = {
                "symbol": symbol,
                "signal_type": trade.get("signal_type", "Neutral"),
                "confidence": trade.get("confidence", 0.5),
                "entry_price": trade.get("entry_price", 0),
                "exit_price": trade.get("exit_price", 0),
                "pnl_pct": trade.get("pnl_pct", 0),
                "trade_date": trade.get("trade_date", ""),
                "profitable": 1 if (trade.get("pnl_pct", 0) or 0) > 0 else 0,
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
    parser.add_argument("--symbols", nargs="+", default=DEFAULT_SYMBOLS, help="Symbols to backtest")
    parser.add_argument("--days", type=int, default=365, help="Backtest lookback days")
    parser.add_argument("--output", default="./data/signal_training_data.json", help="Output file")
    parser.add_argument("--delay", type=float, default=2.0, help="Delay between API calls (seconds)")
    parser.add_argument("--api-base", default=API_BASE, help="API base URL")
    args = parser.parse_args()

    global API_BASE
    API_BASE = args.api_base

    n = generate_training_data(args.symbols, args.days, args.output, args.delay)
    print(f"\nDone! Generated {n} training samples.")


if __name__ == "__main__":
    main()
