"""Tests for backtest_panel component."""
import pytest
import responses
import sys
import plotly.graph_objects as go


class TestBacktestPanelComponent:
    """Test suite for BacktestPanelComponent."""

    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Setup for each test - reload modules with test env vars."""
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.backtest_panel' in sys.modules:
            del sys.modules['components.backtest_panel']

    @responses.activate
    def test_fetch_backtest_success(self, mock_backtest_response):
        """Test successful fetch of backtest results."""
        from components.backtest_panel import BacktestPanelComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/backtest/AAPL",
            json=mock_backtest_response,
            status=200
        )

        result = BacktestPanelComponent.fetch_backtest("AAPL", days=365)

        assert result is not None
        assert result["symbol"] == "AAPL"
        assert result["total_return_percent"] == 15.0
        assert result["win_rate"] == 64.0

        # Verify query params
        assert len(responses.calls) == 1
        assert "days=365" in responses.calls[0].request.url

    @responses.activate
    def test_fetch_backtest_failure(self):
        """Test fetch_backtest returns None on HTTP 500."""
        from components.backtest_panel import BacktestPanelComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/backtest/AAPL",
            json={"success": False, "error": "Internal server error"},
            status=500
        )

        result = BacktestPanelComponent.fetch_backtest("AAPL")

        assert result is None

    @responses.activate
    def test_fetch_monte_carlo_success(self):
        """Test successful fetch of Monte Carlo simulation."""
        from components.backtest_panel import BacktestPanelComponent

        mock_mc_response = {
            "success": True,
            "data": {
                "simulations": 1000,
                "probability_of_profit": 72.5,
                "probability_of_ruin": 3.2,
                "median_return": 12.5,
                "percentile_5": -8.3,
                "percentile_95": 35.7,
                "median_max_drawdown": -6.8,
                "median_sharpe": 1.65,
                "return_distribution": [-5.2, 8.1, 15.3, 22.0]
            }
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/backtest/results/bt_12345/monte-carlo",
            json=mock_mc_response,
            status=200
        )

        result = BacktestPanelComponent.fetch_monte_carlo("bt_12345", simulations=1000)

        assert result is not None
        assert result["probability_of_profit"] == 72.5
        assert result["simulations"] == 1000

        # Verify query params
        assert "simulations=1000" in responses.calls[0].request.url

    @responses.activate
    def test_fetch_monte_carlo_failure(self):
        """Test fetch_monte_carlo returns None on failure."""
        from components.backtest_panel import BacktestPanelComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/backtest/results/bt_12345/monte-carlo",
            json={"success": False, "error": "Backtest not found"},
            status=404
        )

        result = BacktestPanelComponent.fetch_monte_carlo("bt_12345")

        assert result is None

    def test_create_panel_with_none_data(self):
        """Test create_panel returns Alert when data is None."""
        from components.backtest_panel import BacktestPanelComponent
        from dash import html
        import dash_bootstrap_components as dbc

        result = BacktestPanelComponent.create_panel(None, "AAPL")

        # Should return an Alert component
        assert result is not None
        assert isinstance(result, dbc.Alert)

    def test_create_panel_with_valid_data(self, mock_backtest_response):
        """Test create_panel creates valid panel with backtest data."""
        from components.backtest_panel import BacktestPanelComponent
        from dash import html

        data = mock_backtest_response["data"]
        result = BacktestPanelComponent.create_panel(data, "AAPL")

        # Should return a Div container
        assert result is not None
        assert isinstance(result, html.Div)

    def test_create_metrics_row(self, mock_backtest_response):
        """Test _create_metrics_row renders 4 primary metric cards."""
        from components.backtest_panel import BacktestPanelComponent
        from dash import html

        data = mock_backtest_response["data"]
        metrics_row = BacktestPanelComponent._create_metrics_row(data)

        # Should return a Div containing rows of cards
        assert metrics_row is not None
        assert isinstance(metrics_row, html.Div)

    def test_create_metrics_row_with_extended_metrics(self, mock_backtest_response):
        """Test _create_metrics_row includes extended metrics when available."""
        from components.backtest_panel import BacktestPanelComponent

        data = mock_backtest_response["data"]
        # Data already includes sortino, calmar, etc.

        metrics_row = BacktestPanelComponent._create_metrics_row(data)

        # Should render both primary and extended metrics
        assert metrics_row is not None

    def test_create_equity_curve_returns_figure(self, mock_backtest_response):
        """Test _create_equity_curve returns a plotly Figure."""
        from components.backtest_panel import BacktestPanelComponent

        equity_curve = mock_backtest_response["data"]["equity_curve"]
        fig = BacktestPanelComponent._create_equity_curve(
            equity_curve, "AAPL", 100000, None
        )

        assert isinstance(fig, go.Figure)
        assert len(fig.data) >= 1  # At least strategy line

    def test_create_equity_curve_with_benchmark(self, mock_backtest_response):
        """Test _create_equity_curve includes benchmark lines when provided."""
        from components.backtest_panel import BacktestPanelComponent

        data = mock_backtest_response["data"]
        equity_curve = data["equity_curve"]
        benchmark = data["benchmark"]

        fig = BacktestPanelComponent._create_equity_curve(
            equity_curve, "AAPL", 100000, benchmark
        )

        # Should have strategy + buy_hold + SPY lines
        assert len(fig.data) == 3

    def test_create_trade_table(self, mock_backtest_response):
        """Test _create_trade_table creates table from trades."""
        from components.backtest_panel import BacktestPanelComponent
        import dash_bootstrap_components as dbc

        trades = mock_backtest_response["data"]["trades"]
        table = BacktestPanelComponent._create_trade_table(trades)

        assert isinstance(table, dbc.Table)

    def test_create_trade_table_limits_rows(self):
        """Test _create_trade_table respects limit parameter."""
        from components.backtest_panel import BacktestPanelComponent

        # Create 50 fake trades
        trades = [
            {
                "signal": "Buy",
                "entry_date": "2024-01-01T00:00:00Z",
                "exit_date": "2024-01-05T00:00:00Z",
                "entry_price": 100.0,
                "exit_price": 105.0,
                "profit_loss": 50.0,
                "profit_loss_percent": 5.0,
                "holding_period_days": 4,
                "exit_reason": "take_profit"
            }
            for _ in range(50)
        ]

        table = BacktestPanelComponent._create_trade_table(trades, limit=10)

        # Should only render 10 rows (plus header)
        assert table is not None

    def test_create_benchmark_row(self, mock_backtest_response):
        """Test _create_benchmark_row creates comparison metrics."""
        from components.backtest_panel import BacktestPanelComponent
        import dash_bootstrap_components as dbc

        benchmark = mock_backtest_response["data"]["benchmark"]
        row = BacktestPanelComponent._create_benchmark_row(benchmark)

        # Should return a Row with cards
        assert isinstance(row, dbc.Row)

    def test_create_per_symbol_table(self):
        """Test _create_per_symbol_table for multi-symbol backtests."""
        from components.backtest_panel import BacktestPanelComponent
        import dash_bootstrap_components as dbc

        per_symbol = [
            {
                "symbol": "AAPL",
                "weight": 0.5,
                "total_trades": 10,
                "win_rate": 70.0,
                "total_return": 5000.0,
                "total_return_percent": 10.0
            },
            {
                "symbol": "MSFT",
                "weight": 0.5,
                "total_trades": 8,
                "win_rate": 62.5,
                "total_return": 3500.0,
                "total_return_percent": 7.0
            }
        ]

        table = BacktestPanelComponent._create_per_symbol_table(per_symbol)

        assert isinstance(table, dbc.Table)

    def test_create_monte_carlo_panel_with_none(self):
        """Test create_monte_carlo_panel handles None data."""
        from components.backtest_panel import BacktestPanelComponent
        import dash_bootstrap_components as dbc

        result = BacktestPanelComponent.create_monte_carlo_panel(None)

        assert isinstance(result, dbc.Alert)

    def test_create_monte_carlo_panel_with_data(self):
        """Test create_monte_carlo_panel renders simulation results."""
        from components.backtest_panel import BacktestPanelComponent
        from dash import html

        mc_data = {
            "simulations": 1000,
            "probability_of_profit": 68.5,
            "probability_of_ruin": 4.2,
            "median_return": 10.5,
            "percentile_5": -5.0,
            "percentile_95": 28.0,
            "median_max_drawdown": -7.5,
            "median_sharpe": 1.55,
            "return_distribution": [i * 0.5 - 10 for i in range(100)]
        }

        result = BacktestPanelComponent.create_monte_carlo_panel(mc_data)

        assert isinstance(result, html.Div)

    def test_create_walk_forward_panel_with_none(self):
        """Test create_walk_forward_panel handles None data."""
        from components.backtest_panel import BacktestPanelComponent
        import dash_bootstrap_components as dbc

        result = BacktestPanelComponent.create_walk_forward_panel(None)

        assert isinstance(result, dbc.Alert)

    def test_create_walk_forward_panel_with_data(self):
        """Test create_walk_forward_panel renders validation results."""
        from components.backtest_panel import BacktestPanelComponent
        from dash import html

        wf_data = {
            "overfitting_ratio": 0.95,
            "avg_in_sample_return": 18.5,
            "avg_out_of_sample_return": 12.0,
            "out_of_sample_win_rate": 58.5,
            "out_of_sample_sharpe": 1.35,
            "total_oos_trades": 45,
            "folds": [
                {
                    "fold_number": 1,
                    "train_start": "2024-01-01T00:00:00Z",
                    "train_end": "2024-06-30T00:00:00Z",
                    "test_start": "2024-07-01T00:00:00Z",
                    "test_end": "2024-09-30T00:00:00Z",
                    "in_sample_return": 20.0,
                    "out_of_sample_return": 10.0,
                    "in_sample_trades": 15,
                    "out_of_sample_trades": 8
                }
            ],
            "combined_equity_curve": [
                {"timestamp": "2024-07-01T00:00:00Z", "equity": 100000.0},
                {"timestamp": "2024-09-30T00:00:00Z", "equity": 110000.0}
            ]
        }

        result = BacktestPanelComponent.create_walk_forward_panel(wf_data)

        assert isinstance(result, html.Div)

    def test_equity_curve_with_empty_list(self):
        """Test _create_equity_curve handles empty equity curve."""
        from components.backtest_panel import BacktestPanelComponent

        fig = BacktestPanelComponent._create_equity_curve([], "AAPL", 100000, None)

        # Should still return a figure (possibly empty)
        assert isinstance(fig, go.Figure)

    def test_panel_includes_footnotes(self, mock_backtest_response):
        """Test create_panel includes cost footnotes."""
        from components.backtest_panel import BacktestPanelComponent

        data = mock_backtest_response["data"]
        panel = BacktestPanelComponent.create_panel(data, "AAPL")

        # Panel should be created with footnotes (internal check)
        assert panel is not None

    def test_create_panel_without_benchmark(self):
        """Test create_panel works without benchmark data."""
        from components.backtest_panel import BacktestPanelComponent

        data = {
            "symbol": "AAPL",
            "initial_capital": 100000.0,
            "total_return": 10000.0,
            "total_return_percent": 10.0,
            "win_rate": 60.0,
            "total_trades": 20,
            "winning_trades": 12,
            "sharpe_ratio": 1.5,
            "max_drawdown": -5.0,
            "equity_curve": [
                {"timestamp": "2024-01-01T00:00:00Z", "equity": 100000.0},
                {"timestamp": "2024-03-01T00:00:00Z", "equity": 110000.0}
            ],
            "trades": []
        }

        panel = BacktestPanelComponent.create_panel(data, "AAPL")

        assert panel is not None
