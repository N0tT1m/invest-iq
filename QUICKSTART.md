# InvestIQ Quick Start Guide

## Prerequisites

1. Install Rust: https://rustup.rs/
2. Get a Polygon.io API key (free tier): https://polygon.io/dashboard/signup
3. (Optional) Install Docker for Redis: https://www.docker.com/

## Setup in 5 Minutes

### 1. Configure Environment

```bash
# Copy the example environment file
cp .env.example .env

# Edit .env and add your Polygon API key
# POLYGON_API_KEY=your_key_here
```

### 2. Start Redis (Optional)

```bash
# Start Redis using Docker Compose
docker-compose up -d

# Check if Redis is running
docker ps
```

> **Note**: If you don't start Redis, the API will use in-memory caching automatically.

### 3. Build the Project

```bash
# Build in release mode for best performance
cargo build --release
```

### 4. Run the API Server

```bash
# Start the API server
cargo run --release --bin api-server

# You should see:
# ✅ Connected to Redis at redis://localhost:6379
# 🚀 API Server starting on 0.0.0.0:3000
```

### 5. Test the API

Open a new terminal and test the API:

```bash
# Health check
curl http://localhost:3000/health

# Analyze a stock (e.g., Apple)
curl http://localhost:3000/api/analyze/AAPL | jq

# Get historical bars
curl "http://localhost:3000/api/bars/AAPL?timeframe=1d&days=90" | jq

# Get ticker details
curl http://localhost:3000/api/ticker/AAPL | jq
```

## Using the Discord Bot

### 1. Create a Discord Bot

1. Go to https://discord.com/developers/applications
2. Click "New Application"
3. Go to the "Bot" tab and click "Add Bot"
4. Copy the bot token
5. Under "Privileged Gateway Intents", enable "Message Content Intent"
6. Go to OAuth2 > URL Generator
7. Select scopes: `bot`
8. Select permissions: `Send Messages`, `Read Messages/View Channels`
9. Copy the generated URL and invite the bot to your server

### 2. Configure and Run

```bash
# Add Discord token to .env
# DISCORD_BOT_TOKEN=your_token_here

# Run the bot
cargo run --release --bin discord-bot
```

### 3. Use the Bot

In Discord:
```
!iq analyze AAPL
!iq help
```

## Building a Dash Frontend

Here's a minimal example to get you started:

### 1. Install Dependencies

```bash
pip install dash plotly pandas requests
```

### 2. Create `app.py`

```python
import dash
from dash import dcc, html, Input, Output
import plotly.graph_objects as go
import requests
import pandas as pd

app = dash.Dash(__name__)

app.layout = html.Div([
    html.H1("InvestIQ Stock Analysis"),

    html.Div([
        dcc.Input(id='symbol-input', type='text', value='AAPL', placeholder='Enter symbol'),
        html.Button('Analyze', id='analyze-button', n_clicks=0),
    ]),

    html.Div(id='analysis-output'),
    dcc.Graph(id='stock-chart'),
])

@app.callback(
    [Output('analysis-output', 'children'),
     Output('stock-chart', 'figure')],
    [Input('analyze-button', 'n_clicks')],
    [Input('symbol-input', 'value')]
)
def update_analysis(n_clicks, symbol):
    if not symbol:
        return "Enter a symbol", {}

    # Get analysis
    analysis_response = requests.get(f'http://localhost:3000/api/analyze/{symbol}')
    analysis = analysis_response.json()

    # Get bars for chart
    bars_response = requests.get(f'http://localhost:3000/api/bars/{symbol}?timeframe=1d&days=90')
    bars_data = bars_response.json()

    # Format analysis output
    if analysis['success']:
        data = analysis['data']
        output = html.Div([
            html.H2(f"Analysis for {symbol}"),
            html.H3(f"Signal: {data['overall_signal']}"),
            html.P(f"Recommendation: {data['recommendation']}"),
            html.Hr(),
            html.H4("Technical Analysis"),
            html.P(data['technical']['reason'] if data['technical'] else "N/A"),
            html.H4("Fundamental Analysis"),
            html.P(data['fundamental']['reason'] if data['fundamental'] else "N/A"),
        ])
    else:
        output = html.Div([html.P(f"Error: {analysis['error']}")])

    # Create candlestick chart
    if bars_data['success']:
        bars = bars_data['data']
        df = pd.DataFrame(bars)

        fig = go.Figure(data=[go.Candlestick(
            x=df['timestamp'],
            open=df['open'],
            high=df['high'],
            low=df['low'],
            close=df['close']
        )])

        fig.update_layout(
            title=f'{symbol} Price Chart',
            yaxis_title='Price',
            xaxis_title='Date'
        )
    else:
        fig = {}

    return output, fig

if __name__ == '__main__':
    app.run_server(debug=True, port=8050)
```

### 3. Run the Dash App

```bash
python app.py
```

Visit http://localhost:8050 in your browser!

## API Endpoints Reference

### GET /health
Health check endpoint

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "service": "invest-iq-api"
  }
}
```

### GET /api/analyze/:symbol
Get comprehensive stock analysis

**Parameters:**
- `cache_ttl` (optional): Cache TTL in seconds (default: 300)

**Example:**
```bash
curl "http://localhost:3000/api/analyze/AAPL?cache_ttl=600"
```

### GET /api/bars/:symbol
Get historical price data

**Parameters:**
- `timeframe` (optional): 1m, 5m, 15m, 30m, 1h, 4h, 1d (default), 1w, 1M
- `days` (optional): Number of days to fetch (default: 90)

**Example:**
```bash
curl "http://localhost:3000/api/bars/AAPL?timeframe=1h&days=30"
```

### GET /api/ticker/:symbol
Get ticker details

**Example:**
```bash
curl http://localhost:3000/api/ticker/AAPL
```

## Performance Tips

1. **Use Redis for production** - Enables distributed caching across multiple API instances
2. **Adjust cache TTL** - Balance between fresh data and API rate limits
3. **Use appropriate timeframes** - Shorter timeframes consume more API quota
4. **Enable release mode** - Always use `--release` flag for production

## Troubleshooting

### "POLYGON_API_KEY must be set"
- Make sure you've created a `.env` file
- Add your Polygon API key: `POLYGON_API_KEY=your_key_here`

### "Failed to connect to Redis"
- The API will automatically fall back to in-memory caching
- To use Redis, start it with: `docker-compose up -d`

### "Insufficient data" error
- Some stocks may not have enough historical data
- Try a more liquid stock like AAPL, MSFT, TSLA, etc.

### Rate limiting from Polygon
- Free tier has 5 API calls/minute
- Increase cache TTL to reduce API calls
- Consider upgrading your Polygon plan

## Next Steps

- Add more technical indicators
- Implement backtesting strategies
- Add real-time WebSocket streaming
- Create custom analysis strategies
- Build a full-featured web dashboard

## Support

For issues and questions:
- Check the main README.md
- Review the code documentation
- Open an issue on GitHub

Happy analyzing! 📈
