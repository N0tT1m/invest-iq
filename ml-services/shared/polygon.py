"""Polygon API utilities shared across ML services."""
import os
import requests
from typing import List
from loguru import logger

try:
    import invest_iq_data
    _USE_RUST_FETCHER = True
except ImportError:
    _USE_RUST_FETCHER = False

POLYGON_BASE = "https://api.polygon.io"


def fetch_active_tickers(
    api_key: str = "",
    market: str = "stocks",
    ticker_type: str = "CS",
) -> List[str]:
    """Fetch all active US stock tickers from Polygon /v3/reference/tickers.

    Paginates automatically until exhausted. Returns sorted list of ticker symbols.
    Uses Rust concurrent fetcher if available, falls back to sequential Python.

    Args:
        api_key: Polygon API key (falls back to POLYGON_API_KEY env var)
        market: Market type (stocks, crypto, fx, otc)
        ticker_type: Ticker type (CS=common stock, ETF, etc.)

    Returns:
        List of ticker symbols sorted alphabetically
    """
    api_key = api_key or os.environ.get("POLYGON_API_KEY", "")
    if not api_key:
        raise ValueError("POLYGON_API_KEY not set")

    if _USE_RUST_FETCHER:
        logger.info("Using Rust fetcher for active tickers")
        return invest_iq_data.fetch_active_tickers(api_key, market, ticker_type)

    tickers = []
    cursor = None
    page_limit = 1000

    while True:
        params = {
            "apiKey": api_key,
            "market": market,
            "active": "true",
            "type": ticker_type,
            "limit": str(page_limit),
            "order": "asc",
            "sort": "ticker",
        }
        if cursor:
            params["cursor"] = cursor

        resp = requests.get(f"{POLYGON_BASE}/v3/reference/tickers", params=params, timeout=30)
        resp.raise_for_status()
        data = resp.json()

        results = data.get("results", [])
        if not results:
            break

        for item in results:
            ticker = item.get("ticker", "")
            # Skip tickers with special characters (warrants, units, etc.)
            if ticker and "." not in ticker and "/" not in ticker and len(ticker) <= 5:
                tickers.append(ticker)

        # Follow pagination
        next_url = data.get("next_url")
        if not next_url:
            break

        # Extract cursor from next_url
        from urllib.parse import urlparse, parse_qs
        parsed = urlparse(next_url)
        qs = parse_qs(parsed.query)
        cursor = qs.get("cursor", [None])[0]
        if not cursor:
            break

    logger.info(f"Fetched {len(tickers)} active tickers from Polygon")
    return sorted(tickers)
