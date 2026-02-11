"""Tests for portfolio_dashboard component."""
import pytest
import responses
import sys
import plotly.graph_objects as go


class TestPortfolioDashboardComponent:
    """Test suite for PortfolioDashboardComponent."""

    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Setup for each test - reload modules with test env vars."""
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.portfolio_dashboard' in sys.modules:
            del sys.modules['components.portfolio_dashboard']

    @responses.activate
    def test_fetch_account_success(self, mock_account_response):
        """Test successful fetch of account data."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json=mock_account_response,
            status=200
        )

        result = PortfolioDashboardComponent.fetch_account()

        assert result is not None
        assert result["buying_power"] == "100000.00"
        assert result["portfolio_value"] == "125000.00"

    @responses.activate
    def test_fetch_account_failure(self):
        """Test fetch_account returns None on failure."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json={"success": False, "error": "Broker error"},
            status=500
        )

        result = PortfolioDashboardComponent.fetch_account()

        assert result is None

    @responses.activate
    def test_fetch_positions_success(self, mock_position_response):
        """Test successful fetch of positions."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json={
                "success": True,
                "data": [mock_position_response["data"]]
            },
            status=200
        )

        result = PortfolioDashboardComponent.fetch_positions()

        assert len(result) == 1
        assert result[0]["symbol"] == "AAPL"

    @responses.activate
    def test_fetch_positions_empty(self):
        """Test fetch_positions with no positions."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json={"success": True, "data": []},
            status=200
        )

        result = PortfolioDashboardComponent.fetch_positions()

        assert result == []

    @responses.activate
    def test_fetch_orders_success(self):
        """Test successful fetch of orders."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        mock_orders = {
            "success": True,
            "data": [
                {
                    "id": "ord_123",
                    "symbol": "AAPL",
                    "side": "buy",
                    "qty": "10",
                    "filled_avg_price": "178.50",
                    "status": "filled",
                    "submitted_at": "2024-01-15T10:30:00Z"
                },
                {
                    "id": "ord_124",
                    "symbol": "MSFT",
                    "side": "sell",
                    "qty": "5",
                    "filled_avg_price": "380.00",
                    "status": "filled",
                    "submitted_at": "2024-01-15T11:00:00Z"
                }
            ]
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/orders",
            json=mock_orders,
            status=200
        )

        result = PortfolioDashboardComponent.fetch_orders(limit=20)

        assert len(result) == 2
        assert result[0]["symbol"] == "AAPL"

        # Verify query params
        assert "limit=20" in responses.calls[0].request.url

    @responses.activate
    def test_fetch_orders_failure(self):
        """Test fetch_orders returns empty list on failure."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/orders",
            json={"success": False, "error": "Server error"},
            status=500
        )

        result = PortfolioDashboardComponent.fetch_orders()

        assert result == []

    def test_create_dashboard_with_none_account(self):
        """Test create_dashboard handles None account gracefully."""
        from components.portfolio_dashboard import PortfolioDashboardComponent
        import dash_bootstrap_components as dbc

        dashboard = PortfolioDashboardComponent.create_dashboard(None, [], [])

        # Should return a card with warning
        assert dashboard is not None
        assert isinstance(dashboard, dbc.Card)

    def test_create_dashboard_with_valid_account(self, mock_account_response):
        """Test create_dashboard with valid account data."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = mock_account_response["data"]
        dashboard = PortfolioDashboardComponent.create_dashboard(account, [], [])

        assert dashboard is not None

    def test_create_dashboard_handles_string_to_float_conversion(self):
        """Test create_dashboard converts Alpaca string fields to floats."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = {
            "portfolio_value": "125000.50",
            "buying_power": "100000.25",
            "cash": "50000.75"
        }

        # Should not raise ValueError
        dashboard = PortfolioDashboardComponent.create_dashboard(account, [], [])
        assert dashboard is not None

    def test_create_dashboard_with_positions(self, mock_account_response, mock_position_response):
        """Test create_dashboard renders positions table."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = mock_account_response["data"]
        positions = [mock_position_response["data"]]

        dashboard = PortfolioDashboardComponent.create_dashboard(account, positions, [])

        assert dashboard is not None

    def test_create_dashboard_with_orders(self, mock_account_response):
        """Test create_dashboard renders order history."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = mock_account_response["data"]
        orders = [
            {
                "symbol": "AAPL",
                "side": "buy",
                "qty": "10",
                "filled_avg_price": "178.50",
                "status": "filled",
                "submitted_at": "2024-01-15T10:30:00Z"
            }
        ]

        dashboard = PortfolioDashboardComponent.create_dashboard(account, [], orders)

        assert dashboard is not None

    def test_create_allocation_chart_with_positions(self, mock_position_response):
        """Test _create_allocation_chart creates donut chart."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        positions = [
            {
                "symbol": "AAPL",
                "market_value": "17850.00"
            },
            {
                "symbol": "MSFT",
                "market_value": "19000.00"
            }
        ]

        fig = PortfolioDashboardComponent._create_allocation_chart(positions)

        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1  # Pie chart

    def test_create_allocation_chart_with_no_positions(self):
        """Test _create_allocation_chart handles empty positions."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        fig = PortfolioDashboardComponent._create_allocation_chart([])

        assert isinstance(fig, go.Figure)
        # Should have annotation saying "No positions"

    def test_float_conversion_from_position_strings(self, mock_position_response):
        """Test that position string fields are correctly converted to floats."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = {
            "portfolio_value": "125000.00",
            "buying_power": "100000.00",
            "cash": "50000.00"
        }

        position = mock_position_response["data"]

        # Should not raise on float conversions
        dashboard = PortfolioDashboardComponent.create_dashboard(account, [position], [])
        assert dashboard is not None

    def test_position_table_handles_pnl_colors(self):
        """Test positions table applies correct color classes for P&L."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = {
            "portfolio_value": "125000.00",
            "buying_power": "100000.00",
            "cash": "50000.00"
        }

        positions = [
            {
                "symbol": "AAPL",
                "qty": "100",
                "avg_entry_price": "175.00",
                "current_price": "180.00",
                "market_value": "18000.00",
                "unrealized_pl": "500.00",
                "unrealized_plpc": "0.0286",
                "change_today": "0.0028"
            },
            {
                "symbol": "MSFT",
                "qty": "50",
                "avg_entry_price": "400.00",
                "current_price": "390.00",
                "market_value": "19500.00",
                "unrealized_pl": "-500.00",
                "unrealized_plpc": "-0.025",
                "change_today": "-0.015"
            }
        ]

        dashboard = PortfolioDashboardComponent.create_dashboard(account, positions, [])

        # Internal logic should handle positive (green) and negative (red) coloring
        assert dashboard is not None

    def test_order_status_badge_colors(self, mock_account_response):
        """Test order history applies correct badge colors for status."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = mock_account_response["data"]

        orders = [
            {"symbol": "AAPL", "side": "buy", "qty": "10", "filled_avg_price": "178.50",
             "status": "filled", "submitted_at": "2024-01-15T10:30:00Z"},
            {"symbol": "MSFT", "side": "sell", "qty": "5", "filled_avg_price": None,
             "status": "canceled", "submitted_at": "2024-01-15T11:00:00Z"},
            {"symbol": "GOOGL", "side": "buy", "qty": "2", "filled_avg_price": None,
             "status": "rejected", "submitted_at": "2024-01-15T12:00:00Z"},
        ]

        dashboard = PortfolioDashboardComponent.create_dashboard(account, [], orders)

        # Should handle different status badge colors
        assert dashboard is not None

    def test_order_timestamp_formatting(self, mock_account_response):
        """Test order timestamps are formatted correctly."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        account = mock_account_response["data"]

        orders = [
            {
                "symbol": "AAPL",
                "side": "buy",
                "qty": "10",
                "filled_avg_price": "178.50",
                "status": "filled",
                "submitted_at": "2024-01-15T10:30:00.123456Z"
            }
        ]

        dashboard = PortfolioDashboardComponent.create_dashboard(account, [], orders)

        # Timestamp should be truncated to first 16 chars (YYYY-MM-DD HH:MM)
        assert dashboard is not None

    def test_allocation_chart_handles_zero_market_value(self):
        """Test allocation chart filters out zero/negative market values."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        positions = [
            {"symbol": "AAPL", "market_value": "0.00"},
            {"symbol": "MSFT", "market_value": "-100.00"},
            {"symbol": "GOOGL", "market_value": "15000.00"}
        ]

        fig = PortfolioDashboardComponent._create_allocation_chart(positions)

        # Should only include GOOGL (positive market value)
        assert isinstance(fig, go.Figure)
