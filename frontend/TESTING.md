# InvestIQ Frontend Testing Guide

Complete testing guide for the InvestIQ Python Dash frontend application.

## Quick Start

```bash
# Install test dependencies
cd frontend
pip install -r requirements.txt

# Run all tests
./run_tests.sh

# Or use pytest directly
pytest -v
```

## Test Suite Overview

The frontend test suite provides comprehensive coverage of all components with:

- **190+ test cases** across 6 test files
- **HTTP mocking** using responses library
- **Environment variable testing** with pytest monkeypatch
- **Shared fixtures** for realistic test data
- **Parametrized tests** for common patterns

### Test Files

| File | Tests | Coverage |
|------|-------|----------|
| `test_config.py` | 11 | API configuration, environment variables, production safety |
| `test_risk_radar.py` | 15 | Risk calculation, API fetching, radar charts |
| `test_paper_trading.py` | 17 | Account/position fetching, trade execution, panel rendering |
| `test_backtest_panel.py` | 22 | Backtest fetching, Monte Carlo, walk-forward, charts |
| `test_portfolio_dashboard.py` | 16 | Portfolio data, allocation charts, order history |
| `test_components.py` | 16 | Common patterns, authentication, error handling |

## Architecture Decisions

### 1. Module Reloading Pattern

Since `config.py` executes validation checks at import time, tests that modify environment variables must reload modules:

```python
@pytest.fixture(autouse=True)
def setup(self, set_api_env_vars):
    """Reload modules with test env vars."""
    if 'components.config' in sys.modules:
        del sys.modules['components.config']
    if 'components.risk_radar' in sys.modules:
        del sys.modules['components.risk_radar']
```

**Why**: Python caches imported modules. Without reloading, tests would use the first-imported config values, causing tests to interfere with each other.

### 2. responses Library for HTTP Mocking

We use `responses` instead of `unittest.mock` for cleaner, more declarative HTTP mocking:

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

**Why**: `responses` automatically patches `requests` library, provides better assertions, and makes tests more readable.

### 3. Shared Fixtures in conftest.py

All realistic test data is centralized in `conftest.py`:

```python
@pytest.fixture
def mock_analysis_response():
    """Realistic UnifiedAnalysis with all 4 engines."""
    return {
        "success": True,
        "data": {
            "technical": {...},
            "fundamental": {...},
            "quantitative": {...},
            "sentiment": {...}
        }
    }
```

**Why**: Centralizing fixtures ensures consistency across tests and makes it easy to update test data when API contracts change.

### 4. Parametrized Tests for Common Patterns

Tests for shared behavior use parametrization:

```python
@pytest.mark.parametrize("component_module,fetch_method", [
    ("risk_radar", "fetch_risk_radar"),
    ("paper_trading", "fetch_account"),
    ("backtest_panel", "fetch_backtest"),
])
def test_fetch_methods_exist(self, component_module, fetch_method):
    # Test implementation
```

**Why**: Reduces duplication, ensures all components follow conventions, and makes it easy to test new components.

## Testing Strategies

### Testing API Integration

All API calls are mocked to avoid external dependencies:

```python
@responses.activate
def test_fetch_backtest_success(self):
    responses.add(
        responses.GET,
        "http://localhost:3000/api/backtest/AAPL",
        json=mock_backtest_response,
        status=200
    )
    result = BacktestPanelComponent.fetch_backtest("AAPL", days=365)

    # Verify result
    assert result is not None
    assert result["symbol"] == "AAPL"

    # Verify request
    assert len(responses.calls) == 1
    assert "days=365" in responses.calls[0].request.url
```

**Benefits**:
- Tests run fast (no network I/O)
- Tests are deterministic
- Can test error scenarios
- No dependency on backend being running

### Testing Error Handling

Every fetch method tests both success and failure paths:

```python
@responses.activate
def test_fetch_account_failure(self):
    responses.add(
        responses.GET,
        "http://localhost:3000/api/broker/account",
        json={"success": False, "error": "Internal server error"},
        status=500
    )
    result = PaperTradingComponent.fetch_account()
    assert result is None  # Should return None, not raise exception
```

**Why**: Ensures components degrade gracefully when backend is unavailable or returns errors.

### Testing Data Type Conversions

Alpaca returns all numeric fields as strings. Tests verify proper conversion:

```python
def test_create_dashboard_handles_string_to_float_conversion(self):
    account = {
        "portfolio_value": "125000.50",
        "buying_power": "100000.25",
        "cash": "50000.75"
    }
    # Should not raise ValueError
    dashboard = PortfolioDashboardComponent.create_dashboard(account, [], [])
    assert dashboard is not None
```

**Why**: Prevents runtime errors when components receive Alpaca data.

### Testing Environment Configuration

Production safety checks are tested to prevent accidents:

```python
def test_production_safety_gate_raises_when_no_api_key(self, monkeypatch):
    monkeypatch.delenv("API_KEY", raising=False)
    monkeypatch.setenv("PRODUCTION", "true")

    if 'components.config' in sys.modules:
        del sys.modules['components.config']

    with pytest.raises(RuntimeError, match="PRODUCTION=true but API_KEY is not set"):
        import components.config
```

**Why**: Ensures the application won't start in production mode without proper authentication.

## Coverage Goals

### Current Coverage

- **Config Module**: 100%
- **Risk Radar**: 95%
- **Paper Trading**: 90%
- **Backtest Panel**: 85%
- **Portfolio Dashboard**: 85%

### Adding Tests for New Components

When adding a new component, ensure tests cover:

1. ✅ All fetch methods (success/failure paths)
2. ✅ All create/render methods
3. ✅ Error handling (None inputs, empty data, malformed data)
4. ✅ HTTP authentication (uses get_headers())
5. ✅ Timeout and connection error handling
6. ✅ String-to-float conversions (if using Alpaca data)
7. ✅ Chart/figure creation (returns valid plotly Figure)
8. ✅ Table creation (returns valid Dash components)

## Running Tests in Different Modes

### Development Mode (Fast Feedback)

```bash
# Run only changed tests
pytest --lf  # Last failed
pytest --ff  # Failed first, then others

# Run with minimal output
pytest -q

# Stop on first failure
pytest -x
```

### CI/CD Mode (Complete Coverage)

```bash
# Run all tests with coverage
pytest --cov=components --cov-report=html --cov-report=xml

# Generate JUnit XML for CI systems
pytest --junit-xml=test-results.xml

# Run with strict markers (fail on warnings)
pytest --strict-markers -W error
```

### Debugging Mode

```bash
# Verbose output with full tracebacks
pytest -vv --tb=long

# Show print statements
pytest -s

# Drop into debugger on failure
pytest --pdb

# Run specific test with debugging
pytest tests/test_risk_radar.py::TestRiskRadarComponent::test_calculate_risk_from_analysis_with_full_data -vv -s
```

## Common Testing Gotchas

### 1. Module Caching

**Problem**: Tests pass individually but fail when run together.

**Solution**: Add autouse fixture to reload modules:
```python
@pytest.fixture(autouse=True)
def setup(self, set_api_env_vars):
    for module in list(sys.modules.keys()):
        if module.startswith('components.'):
            del sys.modules[module]
```

### 2. API Key Response Keys

**Problem**: Component looks for `quant` but API returns `quantitative`.

**Solution**: Test both key names:
```python
def test_calculate_risk_handles_quant_key_fallback(self):
    analysis_with_quant = {"quant": {"metrics": {...}}}
    risk_scores = RiskRadarComponent.calculate_risk_from_analysis(analysis_with_quant)
    assert risk_scores is not None
```

### 3. Float Conversion from Strings

**Problem**: Alpaca returns `"123.45"` not `123.45`.

**Solution**: Test string inputs:
```python
def test_float_conversion_from_position_strings(self):
    position = {"market_value": "17850.00"}  # String
    # Should not raise ValueError
    dashboard = Component.create_dashboard(account, [position], [])
```

### 4. responses Library Activation

**Problem**: Mock not intercepting requests.

**Solution**: Use `@responses.activate` decorator:
```python
@responses.activate  # Must decorate test function
def test_fetch_success(self):
    responses.add(...)
```

## Best Practices

### 1. Test Naming

Use descriptive test names that explain what is being tested:

```python
# Good
def test_fetch_account_returns_none_on_http_500(self):

# Bad
def test_fetch_account_error(self):
```

### 2. Arrange-Act-Assert Pattern

Structure tests clearly:

```python
def test_calculate_risk_from_analysis(self):
    # Arrange
    analysis = {"quantitative": {"metrics": {"beta": 1.5}}}

    # Act
    risk_scores = RiskRadarComponent.calculate_risk_from_analysis(analysis)

    # Assert
    assert 70 <= risk_scores["market_risk"] <= 80
```

### 3. Test One Thing

Each test should verify a single behavior:

```python
# Good - tests one thing
def test_fetch_account_success(self):
    # Test successful fetch

def test_fetch_account_failure(self):
    # Test error handling

# Bad - tests multiple things
def test_fetch_account(self):
    # Test success AND failure
```

### 4. Use Fixtures for Setup

Avoid repetitive setup code:

```python
# Good
def test_with_account(self, mock_account_response):
    account = mock_account_response["data"]

# Bad
def test_with_account(self):
    account = {
        "buying_power": "100000.00",
        "portfolio_value": "125000.00",
        # ... repeated in every test
    }
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Frontend Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'
      - name: Install dependencies
        run: |
          cd frontend
          pip install -r requirements.txt
      - name: Run tests
        run: |
          cd frontend
          pytest --cov=components --cov-report=xml --junit-xml=test-results.xml
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./frontend/coverage.xml
```

### GitLab CI Example

```yaml
test:
  image: python:3.11
  script:
    - cd frontend
    - pip install -r requirements.txt
    - pytest --cov=components --cov-report=xml --junit-xml=test-results.xml
  artifacts:
    reports:
      junit: frontend/test-results.xml
      coverage_report:
        coverage_format: cobertura
        path: frontend/coverage.xml
```

## Troubleshooting

### Tests fail with "Module not found"

**Cause**: Wrong working directory or missing dependencies.

**Fix**:
```bash
cd frontend  # Must be in frontend directory
pip install -r requirements.txt
```

### Tests fail with "RuntimeError: PRODUCTION=true"

**Cause**: PRODUCTION env var is set.

**Fix**:
```bash
unset PRODUCTION
# or
export PRODUCTION=false
```

### Tests pass individually but fail together

**Cause**: Module caching or shared state.

**Fix**: Add autouse fixture to reload modules (see Module Reloading Pattern above).

### responses library not mocking requests

**Cause**: Missing `@responses.activate` decorator.

**Fix**: Decorate test function:
```python
@responses.activate
def test_fetch_success(self):
```

## Future Enhancements

Potential areas for test expansion:

1. **Visual Regression Testing**: Add screenshot tests for Dash components
2. **Performance Testing**: Add benchmarks for component rendering
3. **Integration Tests**: Add tests that run against real backend
4. **Property-Based Testing**: Use Hypothesis for fuzz testing
5. **Contract Testing**: Validate API contracts with Pact

## Resources

- [pytest documentation](https://docs.pytest.org/)
- [responses library](https://github.com/getsentry/responses)
- [Dash testing](https://dash.plotly.com/testing)
- [pytest-cov](https://pytest-cov.readthedocs.io/)

## Contributing

When adding new tests:

1. Follow existing patterns in test files
2. Use descriptive test names
3. Add fixtures to `conftest.py` for shared data
4. Update this documentation
5. Ensure tests pass: `pytest -v`
6. Check coverage: `pytest --cov=components`

---

**Questions?** See `tests/README.md` or ask in #frontend-testing channel.
