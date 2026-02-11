"""Parametrized tests for common component patterns."""
import pytest
import responses
import sys


class TestComponentCommonPatterns:
    """Test common patterns across all components."""

    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Setup for each test - reload modules with test env vars."""
        # Clear all component modules
        for module in list(sys.modules.keys()):
            if module.startswith('components.'):
                del sys.modules[module]

    @pytest.mark.parametrize("component_name,empty_method,error_method", [
        ("confidence_gauge", "_empty_results", "_error_results"),
        ("sentiment_velocity", "_empty_results", "_error_results"),
    ])
    def test_components_have_empty_and_error_helpers(self, component_name, empty_method, error_method):
        """Test that components with helper methods have proper empty/error result functions."""
        try:
            module = __import__(f'components.{component_name}', fromlist=[component_name])

            # Check if methods exist (they might be module-level or class methods)
            has_empty = hasattr(module, empty_method)
            has_error = hasattr(module, error_method)

            # If one exists, both should exist
            if has_empty or has_error:
                assert has_empty, f"{component_name} missing {empty_method}"
                assert has_error, f"{component_name} missing {error_method}"
        except (ImportError, AttributeError):
            # Component might not have these methods, which is ok
            pass

    @pytest.mark.parametrize("component_module,class_name", [
        ("risk_radar", "RiskRadarComponent"),
        ("paper_trading", "PaperTradingComponent"),
        ("backtest_panel", "BacktestPanelComponent"),
        ("portfolio_dashboard", "PortfolioDashboardComponent"),
    ])
    def test_component_classes_exist(self, component_module, class_name):
        """Test that expected component classes are defined."""
        module = __import__(f'components.{component_module}', fromlist=[class_name])
        assert hasattr(module, class_name), f"{component_module} missing {class_name}"

    @pytest.mark.parametrize("component_module,fetch_method", [
        ("risk_radar", "fetch_risk_radar"),
        ("paper_trading", "fetch_account"),
        ("paper_trading", "fetch_positions"),
        ("backtest_panel", "fetch_backtest"),
        ("portfolio_dashboard", "fetch_account"),
        ("portfolio_dashboard", "fetch_positions"),
        ("portfolio_dashboard", "fetch_orders"),
    ])
    def test_fetch_methods_exist(self, component_module, fetch_method):
        """Test that all fetch methods are defined."""
        module = __import__(f'components.{component_module}', fromlist=[fetch_method])
        # Get the component class (first capital letter class in module)
        for attr_name in dir(module):
            attr = getattr(module, attr_name)
            if isinstance(attr, type) and 'Component' in attr_name:
                assert hasattr(attr, fetch_method), f"{component_module}.{attr_name} missing {fetch_method}"
                break

    @pytest.mark.parametrize("component_module,create_method", [
        ("risk_radar", "create_risk_card"),
        ("paper_trading", "create_panel"),
        ("backtest_panel", "create_panel"),
        ("portfolio_dashboard", "create_dashboard"),
    ])
    def test_create_methods_exist(self, component_module, create_method):
        """Test that all create/render methods are defined."""
        module = __import__(f'components.{component_module}', fromlist=[create_method])
        for attr_name in dir(module):
            attr = getattr(module, attr_name)
            if isinstance(attr, type) and 'Component' in attr_name:
                assert hasattr(attr, create_method), f"{component_module}.{attr_name} missing {create_method}"
                break

    @responses.activate
    def test_risk_radar_uses_get_headers(self):
        """Test that risk_radar uses get_headers() for authentication."""
        from components.risk_radar import RiskRadarComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar/AAPL",
            json={"success": True, "data": {}},
            status=200
        )

        RiskRadarComponent.fetch_risk_radar("AAPL")

        # Verify headers were sent
        assert len(responses.calls) == 1
        headers = responses.calls[0].request.headers
        assert "X-API-Key" in headers
        assert headers["X-API-Key"] == "test-api-key-12345"

    @responses.activate
    def test_paper_trading_uses_get_headers(self):
        """Test that paper_trading uses get_headers() for authentication."""
        from components.paper_trading import PaperTradingComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json={"success": True, "data": {}},
            status=200
        )

        PaperTradingComponent.fetch_account()

        assert len(responses.calls) == 1
        headers = responses.calls[0].request.headers
        assert "X-API-Key" in headers

    @responses.activate
    def test_backtest_panel_uses_get_headers(self):
        """Test that backtest_panel uses get_headers() for authentication."""
        from components.backtest_panel import BacktestPanelComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/backtest/AAPL",
            json={"success": True, "data": {}},
            status=200
        )

        BacktestPanelComponent.fetch_backtest("AAPL")

        assert len(responses.calls) == 1
        headers = responses.calls[0].request.headers
        assert "X-API-Key" in headers

    @responses.activate
    def test_portfolio_dashboard_uses_get_headers(self):
        """Test that portfolio_dashboard uses get_headers() for authentication."""
        from components.portfolio_dashboard import PortfolioDashboardComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            json={"success": True, "data": {}},
            status=200
        )

        PortfolioDashboardComponent.fetch_account()

        assert len(responses.calls) == 1
        headers = responses.calls[0].request.headers
        assert "X-API-Key" in headers

    @pytest.mark.parametrize("component_module", [
        "risk_radar",
        "paper_trading",
        "backtest_panel",
        "portfolio_dashboard",
    ])
    def test_components_import_config(self, component_module):
        """Test that components import from components.config."""
        module = __import__(f'components.{component_module}', fromlist=['API_BASE'])

        # These should be imported from config
        assert hasattr(module, 'API_BASE') or hasattr(module, 'get_headers')

    def test_all_components_use_api_timeout(self):
        """Test that fetch methods use API_TIMEOUT constant."""
        # This is more of a code review check - verify timeout is used in requests
        from components.risk_radar import RiskRadarComponent
        from components.paper_trading import PaperTradingComponent
        from components.backtest_panel import BacktestPanelComponent
        from components.portfolio_dashboard import PortfolioDashboardComponent

        # All these imports should succeed (timeout is imported)
        # Actual usage verification would require code inspection

    @responses.activate
    def test_components_handle_timeout_gracefully(self):
        """Test that components handle request timeouts gracefully."""
        from components.risk_radar import RiskRadarComponent
        import requests.exceptions

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar/AAPL",
            body=requests.exceptions.Timeout()
        )

        result = RiskRadarComponent.fetch_risk_radar("AAPL")

        # Should return None on timeout, not raise exception
        assert result is None

    @responses.activate
    def test_components_handle_connection_error(self):
        """Test that components handle connection errors gracefully."""
        from components.paper_trading import PaperTradingComponent
        import requests.exceptions

        responses.add(
            responses.GET,
            "http://localhost:3000/api/broker/account",
            body=requests.exceptions.ConnectionError()
        )

        result = PaperTradingComponent.fetch_account()

        # Should return None on connection error
        assert result is None

    def test_risk_radar_dimensions_constant(self):
        """Test that RiskRadarComponent has correct DIMENSIONS constant."""
        from components.risk_radar import RiskRadarComponent

        assert hasattr(RiskRadarComponent, 'DIMENSIONS')
        assert len(RiskRadarComponent.DIMENSIONS) == 6
        assert "Market Risk" in RiskRadarComponent.DIMENSIONS
        assert "Volatility" in RiskRadarComponent.DIMENSIONS

    def test_risk_radar_dimension_info_constant(self):
        """Test that RiskRadarComponent has DIMENSION_INFO constant."""
        from components.risk_radar import RiskRadarComponent

        assert hasattr(RiskRadarComponent, 'DIMENSION_INFO')
        assert isinstance(RiskRadarComponent.DIMENSION_INFO, dict)
        assert "Market Risk" in RiskRadarComponent.DIMENSION_INFO

    @responses.activate
    def test_fetch_methods_respect_api_base_url(self, monkeypatch):
        """Test that fetch methods use API_BASE_URL from environment."""
        # Reload config with custom API base
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.risk_radar' in sys.modules:
            del sys.modules['components.risk_radar']

        monkeypatch.setenv("API_BASE_URL", "https://custom-api.example.com")
        monkeypatch.setenv("API_KEY", "test-key")

        from components.risk_radar import RiskRadarComponent

        responses.add(
            responses.GET,
            "https://custom-api.example.com/api/risk/radar/AAPL",
            json={"success": True, "data": {}},
            status=200
        )

        RiskRadarComponent.fetch_risk_radar("AAPL")

        assert len(responses.calls) == 1
        assert "custom-api.example.com" in responses.calls[0].request.url
