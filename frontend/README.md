# InvestIQ Dash Frontend

A comprehensive, interactive dashboard for stock analysis with real-time technical, fundamental, quantitative, and sentiment analysis.

## Features

### 📊 Interactive Charts
- **Candlestick Chart** with Bollinger Bands overlay
- **Volume Bars** color-coded for price movement
- **RSI Indicator** with overbought/oversold zones
- **MACD Chart** with signal line and histogram
- Fully interactive with zoom, pan, and hover details

### 🔧 Technical Analysis
- Real-time RSI, MACD, Bollinger Bands
- Candlestick pattern detection
- Trend analysis
- Multiple timeframes (1m, 5m, 15m, 1h, 1d, 1w)
- Configurable lookback periods

### 💼 Fundamental Analysis
- P/E Ratio
- Return on Equity (ROE)
- Profit Margins
- Debt-to-Equity Ratio
- Current Ratio (Liquidity)
- Operating Cash Flow

### 🔢 Quantitative Analysis
- Sharpe Ratio (risk-adjusted returns)
- Volatility metrics
- Maximum Drawdown
- Beta (market correlation)
- Value at Risk (VaR 95%)
- Win Rate analysis

### 📰 Sentiment Analysis
- News sentiment scoring
- Positive/Negative/Neutral breakdown
- Article count analysis
- Recency-weighted sentiment

## Installation

### Prerequisites
- Python 3.8 or higher
- InvestIQ API server running on http://localhost:3000

### Setup

1. **Install Dependencies**
```bash
cd frontend
pip install -r requirements.txt
```

2. **Run the Dashboard**
```bash
python app.py
```

3. **Access the Dashboard**
Open your browser to: http://localhost:8050

## Usage Guide

### Basic Analysis

1. **Enter a Symbol**: Type a stock ticker (e.g., AAPL, MSFT, TSLA)
2. **Select Timeframe**: Choose from 1m, 5m, 15m, 1h, 1d, or 1w
3. **Set Lookback Period**: Specify how many days of data to analyze (1-365)
4. **Click Analyze**: Get comprehensive analysis results

### Understanding the Dashboard

#### Overall Signal Card (Top)
- Shows the combined recommendation (Strong Buy → Strong Sell)
- Displays confidence score
- Analysis timestamp

#### Price Chart
- Candlestick chart with volume
- Bollinger Bands (gray bands)
- SMA 20 (orange line)
- Interactive zoom and pan

#### Technical Indicators
- **RSI Chart**: Shows momentum (70+ overbought, 30- oversold)
- **MACD Chart**: Shows trend strength and reversals

#### Analysis Cards
Each card shows:
- Signal strength (Buy/Sell)
- Confidence percentage
- Detailed reasoning
- Key metrics specific to that analysis type

### Tips for Best Results

1. **Liquid Stocks**: Use popular stocks (AAPL, MSFT, GOOGL, TSLA) for best data availability
2. **Timeframes**:
   - Intraday trading: 1m, 5m, 15m
   - Swing trading: 1h, 1d
   - Long-term: 1d, 1w
3. **Lookback Period**:
   - Technical analysis: 90 days minimum
   - Fundamental analysis: Not time-dependent
   - Sentiment analysis: Recent news (30-90 days)

## Customization

### Changing API Endpoint
Edit `app.py` and modify:
```python
API_BASE_URL = "http://localhost:3000"  # Change to your API URL
```

### Changing Port
Run with custom port:
```bash
python app.py
# Edit the last line to: app.run_server(debug=True, host='0.0.0.0', port=YOUR_PORT)
```

### Adding Custom Indicators
You can extend the charts by adding custom technical indicators in the chart creation functions:
- `create_main_chart()` - Add price overlays
- `create_rsi_chart()` - Modify RSI display
- `create_macd_chart()` - Modify MACD display

## Troubleshooting

### "Connection refused" or API errors
- Make sure the Rust API server is running: `cargo run --release --bin api-server`
- Check the API is accessible at http://localhost:3000/health

### "No data available"
- Some stocks may not have complete data
- Try a different, more liquid stock
- Reduce the lookback period

### Charts not displaying
- Check browser console for JavaScript errors
- Try refreshing the page
- Ensure you have a stable internet connection

### Slow performance
- Reduce the lookback period (use 30-90 days instead of 365)
- Use higher timeframes (1d instead of 1m)
- Enable caching on the API server (use Redis)

## Architecture

```
User Browser
     ↓
Dash App (localhost:8050)
     ↓
REST API (localhost:3000)
     ↓
Analysis Engines (Rust)
     ↓
Polygon.io API
```

## Advanced Features

### Auto-Refresh
To add auto-refresh, you can add a `dcc.Interval` component:
```python
dcc.Interval(
    id='interval-component',
    interval=60*1000,  # in milliseconds (60 seconds)
    n_intervals=0
)
```

### Multiple Symbols
You can modify the app to track multiple symbols simultaneously by adding a multi-select dropdown.

### Export Data
Add buttons to export charts as images or data as CSV files.

### Alerts
Integrate with the Rust API to set up price alerts or signal notifications.

## Screenshots

### Main Dashboard
- Overall signal with confidence
- Interactive candlestick chart with Bollinger Bands
- Volume analysis

### Technical Analysis
- RSI with overbought/oversold zones
- MACD with signal crossovers
- Detailed technical signals

### All Analysis Types
- Technical metrics and patterns
- Fundamental financial ratios
- Quantitative risk metrics
- News sentiment breakdown

## Performance

- Initial load: ~2-5 seconds (API call)
- Cached requests: <100ms
- Chart rendering: <500ms
- Smooth interactions at 60fps

## Dependencies

- **Dash**: Web framework
- **Plotly**: Interactive charts
- **Pandas**: Data manipulation
- **Requests**: API communication
- **Bootstrap**: Modern UI components

## Contributing

Feel free to enhance the dashboard:
- Add more chart types (Fibonacci, pivot points, etc.)
- Implement comparison mode (multiple stocks)
- Add export functionality
- Create custom alert systems
- Build mobile-responsive layouts

## License

MIT License - Same as main InvestIQ project

---

Built with ❤️ using Dash & Plotly
