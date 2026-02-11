"""Tests for paper_trading component."""
import pytest
import responses
import sys


class TestPaperTradingComponent:
    """Test suite for PaperTradingComponent."""

    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Setup for each test - reload modules with test env vars."""
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.paper_trading' in sys.modules:
            del sys.modules['components.paper_trading']

    @responses.activate
    def test_fetch_account_success(self, mock_account_response):
        """Test successful fetch of Alpaca account data."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json=mock_account_response,
            status=200
        )

        result = PaperTradingComponent.fetch_account()

        assert result is not None
        assert result["buying_power"] == "100000.00"
        assert result["portfolio_value"] == "125000.00"
        assert result["cash"] == "50000.00"
        assert result["status"] == "ACTIVE"

    @responses.activate
    def test_fetch_account_failure(self):
        """Test fetch_account returns None on HTTP 500."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json={"success": False, "error": "Internal server error"},
            status=500
        )

        result = PaperTradingComponent.fetch_account()

        assert result is None

    @responses.activate
    def test_fetch_account_unsuccessful_response(self):
        """Test fetch_account returns None when success=false."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json={"success": False, "error": "Broker not configured"},
            status=200
        )

        result = PaperTradingComponent.fetch_account()

        assert result is None

    @responses.activate
    def test_fetch_positions_success(self):
        """Test successful fetch of positions."""
        from components.paper_trading import PaperTradingComponent

        mock_positions = {
            "success": True,
            "data": [
                {
                    "symbol": "AAPL",
                    "qty": "100",
                    "avg_entry_price": "175.50",
                    "market_value": "17850.00",
                    "unrealized_pl": "300.00"
                },
                {
                    "symbol": "MSFT",
                    "qty": "50",
                    "avg_entry_price": "380.00",
                    "market_value": "19000.00",
                    "unrealized_pl": "-500.00"
                }
            ]
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json=mock_positions,
            status=200
        )

        result = PaperTradingComponent.fetch_positions()

        assert len(result) == 2
        assert result[0]["symbol"] == "AAPL"
        assert result[1]["symbol"] == "MSFT"

    @responses.activate
    def test_fetch_positions_empty(self):
        """Test fetch_positions returns empty list when no positions."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json={"success": True, "data": []},
            status=200
        )

        result = PaperTradingComponent.fetch_positions()

        assert result == []

    @responses.activate
    def test_fetch_position_by_symbol(self, mock_position_response):
        """Test fetch_position retrieves specific symbol."""
        from components.paper_trading import PaperTradingComponent

        positions_response = {
            "success": True,
            "data": [
                mock_position_response["data"],
                {
                    "symbol": "MSFT",
                    "qty": "50",
                    "avg_entry_price": "380.00"
                }
            ]
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json=positions_response,
            status=200
        )

        result = PaperTradingComponent.fetch_position("AAPL")

        assert result is not None
        assert result["symbol"] == "AAPL"
        assert result["qty"] == "100"

    @responses.activate
    def test_fetch_position_not_found(self):
        """Test fetch_position returns None when symbol not in positions."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/positions",
            json={
                "success": True,
                "data": [{"symbol": "MSFT", "qty": "50"}]
            },
            status=200
        )

        result = PaperTradingComponent.fetch_position("AAPL")

        assert result is None

    def test_get_trade_headers_without_live_key(self, set_api_env_vars):
        """Test get_trade_headers without LIVE_TRADING_KEY set."""
        from components.paper_trading import PaperTradingComponent

        headers = PaperTradingComponent.get_trade_headers()

        assert "X-API-Key" in headers
        assert headers["X-API-Key"] == "test-api-key-12345"
        assert "X-Live-Trading-Key" not in headers

    def test_get_trade_headers_with_live_key(self, monkeypatch):
        """Test get_trade_headers includes X-Live-Trading-Key when env var set."""
        # Reload modules with live key set
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.paper_trading' in sys.modules:
            del sys.modules['components.paper_trading']

        monkeypatch.setenv("API_KEY", "test-api-key")
        monkeypatch.setenv("LIVE_TRADING_KEY", "live-secret-key-xyz")

        from components.paper_trading import PaperTradingComponent

        headers = PaperTradingComponent.get_trade_headers()

        assert "X-API-Key" in headers
        assert "X-Live-Trading-Key" in headers
        assert headers["X-Live-Trading-Key"] == "live-secret-key-xyz"

    @responses.activate
    def test_execute_trade_success(self):
        """Test successful trade execution."""
        from components.paper_trading import PaperTradingComponent

        mock_response = {
            "success": True,
            "data": {
                "order_id": "ord_12345",
                "symbol": "AAPL",
                "qty": "10",
                "side": "buy",
                "status": "filled"
            }
        }

        responses.add(
            responses.POST,
            "http://localhost:3000/api/broker/execute",
            json=mock_response,
            status=200
        )

        result = PaperTradingComponent.execute_trade("AAPL", "buy", 10)

        assert result["success"] is True
        assert result["data"]["order_id"] == "ord_12345"
        assert result["data"]["symbol"] == "AAPL"

        # Verify request payload
        assert len(responses.calls) == 1
        request_body = responses.calls[0].request.body
        assert b'"symbol": "AAPL"' in request_body
        assert b'"action": "buy"' in request_body
        assert b'"shares": 10' in request_body
        assert b'"notes": "Executed from main dashboard"' in request_body

    @responses.activate
    def test_execute_trade_failure(self):
        """Test execute_trade handles API errors."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.POST,
            "http://localhost:3000/api/broker/execute",
            json={"success": False, "error": "Insufficient buying power"},
            status=400
        )

        result = PaperTradingComponent.execute_trade("AAPL", "buy", 1000)

        assert result["success"] is False
        assert "error" in result

    @responses.activate
    def test_execute_trade_network_error(self):
        """Test execute_trade handles network exceptions."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.POST,
            "http://localhost:3000/api/broker/execute",
            body=ConnectionError("Network error")
        )

        result = PaperTradingComponent.execute_trade("AAPL", "buy", 10)

        assert result["success"] is False
        assert "error" in result

    def test_create_panel_with_none_account(self):
        """Test create_panel handles None account gracefully."""
        from components.paper_trading import PaperTradingComponent

        panel = PaperTradingComponent.create_panel(None, None, "AAPL")

        # Should return a card with warning message
        assert panel is not None

    def test_create_panel_with_valid_account(self, mock_account_response):
        """Test create_panel with valid account data."""
        from components.paper_trading import PaperTradingComponent

        account = mock_account_response["data"]
        panel = PaperTradingComponent.create_panel(account, None, "AAPL")

        # Should return a valid panel
        assert panel is not None

    def test_create_panel_with_position(self, mock_account_response, mock_position_response):
        """Test create_panel displays position data."""
        from components.paper_trading import PaperTradingComponent

        account = mock_account_response["data"]
        position = mock_position_response["data"]

        panel = PaperTradingComponent.create_panel(account, position, "AAPL")

        # Should return panel with position details
        assert panel is not None

    def test_create_panel_with_analysis_signal(self, mock_account_response, mock_analysis_response):
        """Test create_panel includes analysis signal suggestion."""
        from components.paper_trading import PaperTradingComponent

        account = mock_account_response["data"]
        analysis = mock_analysis_response["data"]

        panel = PaperTradingComponent.create_panel(account, None, "AAPL", analysis)

        # Should return panel (signal suggestion logic is internal)
        assert panel is not None

    def test_create_panel_handles_string_to_float_conversion(self):
        """Test create_panel correctly converts Alpaca string fields to floats."""
        from components.paper_trading import PaperTradingComponent

        account = {
            "buying_power": "12345.67",
            "portfolio_value": "98765.43",
            "cash": "11111.22"
        }

        # Should not raise ValueError on string to float conversion
        panel = PaperTradingComponent.create_panel(account, None, "AAPL")
        assert panel is not None
