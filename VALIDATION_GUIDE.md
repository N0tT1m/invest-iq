# InvestIQ Validation & Backtesting Guide

## 🔬 Overview

InvestIQ now includes powerful validation and backtesting features to help you verify the accuracy of our analysis and test historical signal performance.

## Features

### 1. Data Validation
Compare InvestIQ's analysis with industry-standard sources:
- **Technical Indicators**: Compare with Alpha Vantage (RSI, MACD, SMA, etc.)
- **Fundamental Data**: Compare with Yahoo Finance (P/E, ROE, Profit Margins, etc.)
- **Accuracy Metrics**: See percentage differences and overall accuracy scores

### 2. Backtesting
Test the historical performance of InvestIQ's trading signals:
- **Performance Metrics**: Total return, win rate, profit factor, Sharpe ratio
- **Equity Curve**: Visualize portfolio value over time
- **Trade History**: Detailed list of all trades with entry/exit prices and P/L
- **Risk Metrics**: Maximum drawdown, average win/loss

### 3. Multi-Source Comparison
Aggregates data from multiple sources for comprehensive validation:
- Alpha Vantage (Technical)
- Yahoo Finance (Fundamental & Historical)
- Polygon.io (Real-time & News)

## 🚀 Quick Start

### Prerequisites

1. **Required**:
   - Polygon.io API key (already set up)

2. **Optional but Recommended**:
   - Alpha Vantage API key for validation features
     - Get it FREE at: https://www.alphavantage.co/support/#api-key
     - Free tier: 25 API calls/day

### Setup

1. **Add Alpha Vantage Key to .env** (optional):
```bash
ALPHA_VANTAGE_API_KEY=your_key_here
```

2. **Start the Validation Dashboard**:
```bash
cd frontend
python validation_app.py
```

The validation dashboard will run on **http://localhost:8051** (different from main dashboard on 8050)

## 📊 Using the Validation Dashboard

### Tab 1: Data Validation

1. **Enter a stock symbol** (e.g., AAPL)
2. **Click "Validate"**
3. **View results**:
   - Overall accuracy percentage
   - Technical analysis accuracy (compared with Alpha Vantage)
   - Fundamental analysis accuracy (compared with Yahoo Finance)
   - Detailed comparison table showing metric-by-metric differences

**Interpreting Results**:
- ✅ Green checkmark: Our value within acceptable tolerance
- ⚠️ Orange warning: Difference exceeds tolerance threshold
- **Percentage**: Shows how much our value differs from the source

**Example Output**:
```
Overall Accuracy: 87.3%

Technical Analysis: 92.1%
  RSI: Our: 45.2, Alpha Vantage: 44.8 ✓
  SMA-20: Our: $178.45, Alpha Vantage: $178.52 ✓

Fundamental Analysis: 82.5%
  P/E Ratio: Our: 28.5, Yahoo: 28.9 ✓
  ROE: Our: 147.3%, Yahoo: 145.8 ✓
```

### Tab 2: Backtesting

1. **Enter a stock symbol**
2. **Select number of days to backtest** (90-730 days)
3. **Click "Run Backtest"**
4. **View results**:

**Performance Metrics**:
- **Total Return**: Dollar amount gained/lost
- **Total Return %**: Percentage return on investment
- **Win Rate**: Percentage of profitable trades
- **Profit Factor**: Ratio of total wins to total losses (>1 is good)
- **Sharpe Ratio**: Risk-adjusted return (>1 is good, >2 is excellent)

**Equity Curve**:
- Shows portfolio value over time
- Dashed line = initial capital
- Upward trend = profitable strategy

**Trade History**:
- Lists all trades executed during backtest
- Shows entry/exit dates, prices, and P/L for each trade
- Color-coded: green for wins, red for losses

**Example Output**:
```
Total Return: $2,347.89 (+23.48%)
Win Rate: 62.5% (15/24 trades)
Profit Factor: 2.34
Sharpe Ratio: 1.45
Max Drawdown: -8.2%

Average Win: $287.43
Average Loss: -$122.56
Largest Win: $892.14
Largest Loss: -$345.67
```

## 🔌 API Endpoints

If you want to integrate validation into your own tools:

### Validate Analysis
```bash
GET /api/validate/:symbol

Example:
curl http://localhost:3000/api/validate/AAPL

Response:
{
  "success": true,
  "data": {
    "symbol": "AAPL",
    "overall_accuracy": 87.3,
    "technical_comparison": {
      "overall_technical_accuracy": 92.1,
      "rsi_difference": {
        "our_value": 45.2,
        "their_value": 44.8,
        "percentage_difference": 0.89,
        "within_tolerance": true
      },
      ...
    },
    "fundamental_comparison": { ... }
  }
}
```

### Run Backtest
```bash
GET /api/backtest/:symbol?days=365

Example:
curl http://localhost:3000/api/backtest/AAPL?days=365

Response:
{
  "success": true,
  "data": {
    "symbol": "AAPL",
    "total_return": 2347.89,
    "total_return_percent": 23.48,
    "win_rate": 62.5,
    "profit_factor": 2.34,
    "sharpe_ratio": 1.45,
    "trades": [ ... ],
    "equity_curve": [ ... ]
  }
}
```

## 🎯 Understanding Accuracy Metrics

### Overall Accuracy
Weighted average of technical and fundamental accuracy.
- **>90%**: Excellent - our data matches very closely
- **80-90%**: Good - minor differences within acceptable range
- **70-80%**: Fair - some discrepancies, may need investigation
- **<70%**: Concerning - significant differences

### Technical Accuracy
Compares our technical indicators with Alpha Vantage's industry-standard calculations.
- Tolerances:
  - RSI: ±2 points
  - MACD: ±0.5 points
  - SMA: ±1%

### Fundamental Accuracy
Compares our fundamental metrics with Yahoo Finance data.
- Tolerances:
  - P/E Ratio: ±5%
  - ROE: ±5%
  - Profit Margin: ±5%
  - Debt-to-Equity: ±10%
  - Beta: ±0.2

## 📈 Backtesting Best Practices

### 1. Choose Appropriate Time Periods
- **Short-term (90-180 days)**: Test recent market conditions
- **Medium-term (180-365 days)**: Balance recency with sample size
- **Long-term (365-730 days)**: Include various market conditions

### 2. Interpret Results Carefully
- **Look-ahead bias**: Current backtest uses live API, which may have look-ahead bias
- **Market conditions**: Past performance ≠ future results
- **Transaction costs**: Backtest doesn't include commissions/slippage
- **Signal frequency**: More trades = more data, but higher costs in reality

### 3. Use Multiple Stocks
Test on different stocks to see if signals work across various:
- Market caps (large, mid, small)
- Sectors (tech, finance, healthcare, etc.)
- Volatility levels (stable vs. volatile)

### 4. Compare with Buy-and-Hold
Calculate buy-and-hold return for the same period:
```
Buy-and-Hold Return = (Final Price - Initial Price) / Initial Price × 100%
```
Compare with your backtest return to see if signals outperform passive strategy.

## 🔧 Troubleshooting

### Validation Not Available
**Error**: "Validation not enabled. Set ALPHA_VANTAGE_API_KEY."

**Solution**:
1. Get free API key at https://www.alphavantage.co/support/#api-key
2. Add to `.env`: `ALPHA_VANTAGE_API_KEY=your_key`
3. Restart API server: `cargo run --release --bin api-server`

### Rate Limits
**Alpha Vantage**: 25 calls/day (free tier)
- Solution: Upgrade to paid tier or space out validation requests

**Polygon.io**: 5 calls/minute (free tier)
- Backtesting samples data points to avoid hitting limits
- Solution: Upgrade tier for more frequent backtests

### Backtest Too Slow
**Issue**: Backtest takes a long time for long periods

**Explanation**:
- Backtest makes API calls for analysis at multiple time points
- 365-day backtest with daily sampling = ~73 API calls

**Solutions**:
- Use shorter time periods (90-180 days)
- System automatically samples less frequently for longer periods
- Enable Redis caching to speed up repeated analyses

### Inaccurate Backtest Results
**Potential Issues**:
1. **Look-ahead bias**: Current implementation uses live API
2. **Insufficient data**: Symbol may not have enough historical data
3. **Market conditions changed**: Historical patterns may not repeat

**Best Practice**:
- Use backtest as one tool among many
- Compare with other analysis methods
- Test on multiple symbols and time periods

## 💡 Advanced Usage

### Comparing Multiple Stocks
Run validation/backtest on a portfolio of stocks:

```bash
# Validation
for symbol in AAPL MSFT GOOGL; do
  curl http://localhost:3000/api/validate/$symbol | jq '.data.overall_accuracy'
done

# Backtesting
for symbol in AAPL MSFT GOOGL; do
  curl "http://localhost:3000/api/backtest/$symbol?days=365" | jq '.data.total_return_percent'
done
```

### Custom Analysis
Use the validation endpoints in your own scripts:

```python
import requests

def validate_portfolio(symbols):
    results = {}
    for symbol in symbols:
        response = requests.get(f'http://localhost:3000/api/validate/{symbol}')
        data = response.json()
        results[symbol] = data['data']['overall_accuracy']
    return results

portfolio = ['AAPL', 'MSFT', 'GOOGL', 'TSLA']
accuracies = validate_portfolio(portfolio)
print(f"Average accuracy: {sum(accuracies.values()) / len(accuracies):.1f}%")
```

## 📚 Additional Resources

- [Alpha Vantage Documentation](https://www.alphavantage.co/documentation/)
- [Yahoo Finance](https://finance.yahoo.com/)
- [Backtesting Principles](https://www.investopedia.com/terms/b/backtesting.asp)
- [Sharpe Ratio Explained](https://www.investopedia.com/terms/s/sharperatio.asp)

## ⚠️ Important Notes

1. **This is Not Financial Advice**: Validation and backtesting are tools to evaluate the system, not guarantees of future performance.

2. **Data Sources**:
   - Alpha Vantage and Yahoo Finance may have different data than Polygon.io
   - Small differences are normal due to:
     - Different calculation methods
     - Data timing/updates
     - Rounding differences

3. **Backtesting Limitations**:
   - Simplified model (no transaction costs, slippage, or market impact)
   - Look-ahead bias in current implementation
   - Past performance doesn't guarantee future results

4. **Rate Limits**:
   - Free tiers have limited API calls
   - Plan your testing accordingly
   - Consider paid tiers for heavy usage

---

**Need Help?**
- Check logs: `tail -f api-server.log`
- Review API documentation in README.md
- Open an issue on GitHub
