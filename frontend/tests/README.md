# InvestIQ Frontend Test Suite

Complete test coverage for the InvestIQ Python Dash frontend application.

## Test Structure

```
tests/
├── __init__.py                    # Test package marker
├── conftest.py                    # Shared fixtures and test configuration
├── test_config.py                 # Tests for components/config.py
├── test_risk_radar.py             # Tests for RiskRadarComponent
├── test_paper_trading.py          # Tests for PaperTradingComponent
├── test_backtest_panel.py         # Tests for BacktestPanelComponent
├── test_portfolio_dashboard.py    # Tests for PortfolioDashboardComponent
└── test_components.py             # Parametrized tests for common patterns
```

## Installation

Install test dependencies:

```bash
cd frontend
pip install -r requirements.txt
```

This installs:
- `pytest>=7.4` - Test framework
- `pytest-mock>=3.12` - Mocking utilities
- `responses>=0.24` - HTTP request mocking

## Running Tests

### Run all tests
```bash
cd frontend
pytest
```

### Run specific test file
```bash
pytest tests/test_risk_radar.py
```

### Run specific test class
```bash
pytest tests/test_risk_radar.py::TestRiskRadarComponent
```

### Run specific test
```bash
pytest tests/test_risk_radar.py::TestRiskRadarComponent::test_calculate_risk_from_analysis_with_full_data
```

### Run with verbose output
```bash
pytest -v
```

### Run with coverage report
```bash
pytest --cov=components --cov-report=html
```

### Run tests matching pattern
```bash
pytest -k "fetch"  # Run all tests with "fetch" in name
```

## Test Fixtures

Shared fixtures are defined in `conftest.py`:

- **mock_analysis_response**: Realistic UnifiedAnalysis JSON with all 4 engines (technical, fundamental, quantitative, sentiment)
- **mock_account_response**: Alpaca account data with string fields
- **mock_position_response**: Alpaca position data with string fields
- **mock_backtest_response**: Complete backtest result with equity curve, trades, benchmark
- **set_api_env_vars**: Sets test environment variables (API_BASE_URL, API_KEY, API_TIMEOUT)
- **clear_env_vars**: Clears all API-related environment variables
- **responses_mock**: Activates responses library for HTTP mocking

## Test Coverage

### config.py
- ✅ get_headers() returns correct format
- ✅ Production safety gate (PRODUCTION=true requires API_KEY)
- ✅ API_BASE defaults and environment override
- ✅ API_TIMEOUT defaults and environment override
- ✅ API_KEY from API_KEYS comma-separated list

### risk_radar.py
- ✅ calculate_risk_from_analysis() with full data
- ✅ calculate_risk_from_analysis() with empty dict
- ✅ Handles both 'quantitative' and 'quant' keys
- ✅ Risk score clamping to 0-100 range
- ✅ fetch_risk_radar() success/failure paths
- ✅ Sentiment risk with article distribution
- ✅ Event risk from technical confidence

### paper_trading.py
- ✅ fetch_account() success/failure
- ✅ fetch_positions() success/empty
- ✅ fetch_position() by symbol
- ✅ get_trade_headers() with/without LIVE_TRADING_KEY
- ✅ execute_trade() success/failure/network errors
- ✅ create_panel() with None/valid account
- ✅ String to float conversion (Alpaca returns strings)

### backtest_panel.py
- ✅ fetch_backtest() success/failure
- ✅ fetch_monte_carlo() success/failure
- ✅ create_panel() with None/valid data
- ✅ _create_metrics_row() renders cards
- ✅ _create_equity_curve() returns Figure
- ✅ Equity curve with benchmark lines
- ✅ Trade table creation and limits
- ✅ Monte Carlo panel rendering
- ✅ Walk-forward panel rendering

### portfolio_dashboard.py
- ✅ fetch_account()/positions()/orders() success/failure
- ✅ create_dashboard() with None/valid account
- ✅ String to float conversion
- ✅ Positions table rendering
- ✅ Allocation donut chart
- ✅ Order history table
- ✅ P&L color handling
- ✅ Order status badge colors

### Common Patterns (test_components.py)
- ✅ All components use get_headers() for authentication
- ✅ All fetch methods exist and are accessible
- ✅ All create methods exist
- ✅ Components handle timeout/connection errors gracefully
- ✅ Components respect API_BASE_URL environment variable

## Key Testing Patterns

### HTTP Mocking with responses
```python
@responses.activate
def test_fetch_success(self):
    responses.add(
        responses.GET,
        "http://localhost:3000/api/endpoint",
        json={"success": True, "data": {}},
        status=200
    )
    result = Component.fetch_data()
    assert result is not None
```

### Environment Variable Testing
```python
def test_with_env_vars(self, monkeypatch):
    monkeypatch.setenv("API_KEY", "test-key")
    # Reload module to pick up new env vars
    if 'components.config' in sys.modules:
        del sys.modules['components.config']
    from components.config import API_KEY
    assert API_KEY == "test-key"
```

### Module Reloading
Because config.py runs checks at import time, tests that modify environment variables must reload the module:

```python
if 'components.config' in sys.modules:
    del sys.modules['components.config']
# Now import will re-run with new env vars
```

## Gotchas and Important Notes

1. **Alpaca String Fields**: Alpaca returns all numeric fields as strings. Tests verify proper float conversion.

2. **API Response Keys**: Analysis responses use `technical`, `fundamental`, `quantitative`, `sentiment` (NOT `quant`). Components handle both for backwards compatibility.

3. **Win Rate Format**: Backend returns win_rate as 0-100 percentage (not 0-1 float).

4. **Module Reloading**: Tests that modify env vars must reload modules to avoid cached imports.

5. **Production Safety**: Tests verify that PRODUCTION=true without API_KEY raises RuntimeError on import.

6. **Live Trading Key**: X-Live-Trading-Key header is conditionally added only when LIVE_TRADING_KEY env var is set.

## Continuous Integration

To run tests in CI/CD pipeline:

```bash
# Install dependencies
pip install -r requirements.txt

# Run tests with JUnit XML output
pytest --junit-xml=test-results.xml

# Run with coverage
pytest --cov=components --cov-report=xml --cov-report=html
```

## Writing New Tests

When adding a new component:

1. Create `tests/test_<component_name>.py`
2. Add fixtures to `conftest.py` if needed
3. Test all fetch methods (success/failure paths)
4. Test all create/render methods
5. Test error handling (None inputs, empty data)
6. Test string-to-float conversions if using Alpaca data
7. Add parametrized tests to `test_components.py` for common patterns

Example test structure:
```python
class TestNewComponent:
    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Reload modules with test env vars."""
        if 'components.new_component' in sys.modules:
            del sys.modules['components.new_component']

    @responses.activate
    def test_fetch_success(self):
        # Test implementation
        pass

    def test_create_with_none_data(self):
        # Test implementation
        pass
```

## License

Same as parent InvestIQ project.
