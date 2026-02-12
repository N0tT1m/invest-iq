"""Shared pytest fixtures for frontend tests."""
import os
import pytest
import responses


@pytest.fixture
def mock_analysis_response():
    """Realistic UnifiedAnalysis JSON response from /api/analyze endpoint."""
    return {
        "success": True,
        "data": {
            "symbol": "AAPL",
            "overall_signal": "Buy",
            "overall_confidence": 0.75,
            "current_price": 178.50,
            "technical": {
                "signal": "Buy",
                "confidence": 0.80,
                "reason": "Strong momentum with RSI at 65 and price above SMA-20. Volume confirmation on recent breakout.",
                "metrics": {
                    "rsi": 65.2,
                    "macd": 1.5,
                    "macd_signal": 1.2,
                    "bb_width": 0.045,
                    "bb_percent_b": 0.72,
                    "sma_20": 175.30,
                    "sma_50": 170.80,
                    "atr": 3.25,
                    "adx": 28.5,
                    "support": 172.50,
                    "resistance": 182.00
                }
            },
            "fundamental": {
                "signal": "Hold",
                "confidence": 0.65,
                "reason": "P/E ratio of 28.5 is above sector median. Strong FCF and ROIC indicate quality.",
                "metrics": {
                    "pe_ratio": 28.5,
                    "pb_ratio": 8.2,
                    "ps_ratio": 7.1,
                    "fcf_yield": 0.045,
                    "roic": 0.38,
                    "peg_ratio": 1.85,
                    "quality_score": 0.92
                }
            },
            "quantitative": {
                "signal": "Buy",
                "confidence": 0.72,
                "reason": "Sharpe ratio 1.85 with positive alpha vs SPY. VaR within acceptable limits.",
                "metrics": {
                    "sharpe_ratio": 1.85,
                    "sortino_ratio": 2.10,
                    "beta": 1.15,
                    "alpha": 0.025,
                    "volatility": 22.0,
                    "max_drawdown": -15.0,
                    "var_95": -0.032,
                    "win_rate": 0.62,
                    "correlation_spy": 0.78
                }
            },
            "sentiment": {
                "signal": "Buy",
                "confidence": 0.68,
                "reason": "Positive news sentiment with 75% bullish articles. Low sentiment volatility.",
                "metrics": {
                    "sentiment_score": 0.65,
                    "positive_articles": 15,
                    "negative_articles": 5,
                    "neutral_articles": 3,
                    "total_articles": 23,
                    "buzz_score": 0.82,
                    "entity_weight": 1.5
                }
            }
        }
    }


@pytest.fixture
def mock_account_response():
    """Realistic Alpaca account data with string fields."""
    return {
        "success": True,
        "data": {
            "account_number": "PA12345678",
            "status": "ACTIVE",
            "currency": "USD",
            "buying_power": "100000.00",
            "cash": "50000.00",
            "portfolio_value": "125000.00",
            "equity": "125000.00",
            "last_equity": "124500.00",
            "long_market_value": "75000.00",
            "short_market_value": "0.00",
            "initial_margin": "0.00",
            "maintenance_margin": "0.00",
            "sma": "100000.00",
            "daytrade_count": "0",
            "balance_asof": "2024-01-15",
            "created_at": "2024-01-01T00:00:00Z"
        }
    }


@pytest.fixture
def mock_position_response():
    """Realistic Alpaca position data with string fields."""
    return {
        "success": True,
        "data": {
            "asset_id": "b0b6dd9d-8b9b-48a9-ba46-b9d54906e415",
            "symbol": "AAPL",
            "exchange": "NASDAQ",
            "asset_class": "us_equity",
            "qty": "100",
            "avg_entry_price": "175.50",
            "side": "long",
            "market_value": "17850.00",
            "cost_basis": "17550.00",
            "unrealized_pl": "300.00",
            "unrealized_plpc": "0.0171",
            "unrealized_intraday_pl": "50.00",
            "unrealized_intraday_plpc": "0.0028",
            "current_price": "178.50",
            "lastday_price": "178.00",
            "change_today": "0.0028"
        }
    }


@pytest.fixture
def mock_backtest_response():
    """Realistic backtest result data."""
    return {
        "success": True,
        "data": {
            "id": "bt_12345",
            "symbol": "AAPL",
            "initial_capital": 100000.0,
            "final_equity": 115000.0,
            "total_return": 15000.0,
            "total_return_percent": 15.0,
            "total_trades": 25,
            "winning_trades": 16,
            "losing_trades": 9,
            "win_rate": 64.0,
            "sharpe_ratio": 1.85,
            "sortino_ratio": 2.10,
            "calmar_ratio": 1.25,
            "max_drawdown": -8.5,
            "max_consecutive_wins": 5,
            "max_consecutive_losses": 3,
            "exposure_time_percent": 75.5,
            "recovery_factor": 1.76,
            "total_commission_paid": 50.0,
            "total_slippage_cost": 125.0,
            "equity_curve": [
                {"timestamp": "2024-01-01T00:00:00Z", "equity": 100000.0},
                {"timestamp": "2024-01-15T00:00:00Z", "equity": 102500.0},
                {"timestamp": "2024-02-01T00:00:00Z", "equity": 108000.0},
                {"timestamp": "2024-03-01T00:00:00Z", "equity": 115000.0}
            ],
            "trades": [
                {
                    "signal": "Buy",
                    "entry_date": "2024-01-05T00:00:00Z",
                    "exit_date": "2024-01-10T00:00:00Z",
                    "entry_price": 175.00,
                    "exit_price": 180.00,
                    "profit_loss": 500.0,
                    "profit_loss_percent": 2.86,
                    "holding_period_days": 5,
                    "commission_cost": 2.0,
                    "slippage_cost": 5.0,
                    "exit_reason": "take_profit"
                }
            ],
            "benchmark": {
                "buy_hold_return_percent": 12.5,
                "alpha": 2.5,
                "spy_return_percent": 10.0,
                "spy_alpha": 5.0,
                "information_ratio": 0.85,
                "buy_hold_equity_curve": [
                    {"timestamp": "2024-01-01T00:00:00Z", "equity": 100000.0},
                    {"timestamp": "2024-03-01T00:00:00Z", "equity": 112500.0}
                ],
                "spy_equity_curve": [
                    {"timestamp": "2024-01-01T00:00:00Z", "equity": 100000.0},
                    {"timestamp": "2024-03-01T00:00:00Z", "equity": 110000.0}
                ]
            }
        }
    }


@pytest.fixture
def set_api_env_vars(monkeypatch):
    """Set API environment variables for testing."""
    monkeypatch.setenv("API_BASE_URL", "http://localhost:3000")
    monkeypatch.setenv("API_KEY", "test-api-key-12345")
    monkeypatch.setenv("API_TIMEOUT", "30")


@pytest.fixture
def clear_env_vars(monkeypatch):
    """Clear all API-related environment variables."""
    monkeypatch.delenv("API_BASE_URL", raising=False)
    monkeypatch.delenv("API_KEY", raising=False)
    monkeypatch.delenv("API_KEYS", raising=False)
    monkeypatch.delenv("API_TIMEOUT", raising=False)
    monkeypatch.delenv("PRODUCTION", raising=False)
    monkeypatch.delenv("LIVE_TRADING_KEY", raising=False)


@pytest.fixture
def responses_mock():
    """
    Activate responses library for HTTP mocking.
    This is a context manager that automatically starts/stops.
    """
    with responses.RequestsMock() as rsps:
        yield rsps
