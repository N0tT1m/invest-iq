# InvestIQ Quick Reference Card

## 🚀 Quick Start (3 Steps)

```bash
# 1. Setup environment
cp .env.example .env
# Edit .env and add POLYGON_API_KEY

# 2. Start everything
./start-all.sh

# 3. Use it!
# Main Dashboard: http://localhost:8050
# Validation Dashboard: http://localhost:8051
# API: http://localhost:3000
```

## 🔑 Required Setup

### Get Polygon API Key
1. Go to: https://polygon.io/dashboard/signup
2. Sign up (free tier available)
3. Copy your API key
4. Add to `.env`: `POLYGON_API_KEY=your_key_here`

### (Optional) Discord Bot
1. Go to: https://discord.com/developers/applications
2. Create application → Add Bot
3. Enable "Message Content Intent"
4. Copy token
5. Add to `.env`: `DISCORD_BOT_TOKEN=your_token_here`

### (Optional) Validation Features
1. Go to: https://www.alphavantage.co/support/#api-key
2. Get FREE API key
3. Add to `.env`: `ALPHA_VANTAGE_API_KEY=your_key_here`
4. Restart API server to enable validation

## 📊 Using the System

### Dashboard (Web UI)
```
http://localhost:8050

Features:
- Interactive candlestick charts
- Technical indicators (RSI, MACD, Bollinger Bands)
- All 4 analysis types
- Real-time updates
```

### Discord Bot
```
!iq analyze AAPL    # Full analysis + chart
!iq chart TSLA      # Chart only
!iq help            # Show commands
```

### API Endpoints
```bash
# Health check
curl http://localhost:3000/health

# Analyze stock
curl http://localhost:3000/api/analyze/AAPL

# Get price bars
curl "http://localhost:3000/api/bars/AAPL?timeframe=1d&days=90"

# Ticker details
curl http://localhost:3000/api/ticker/AAPL

# Validate analysis (requires Alpha Vantage key)
curl http://localhost:3000/api/validate/AAPL

# Run backtest
curl "http://localhost:3000/api/backtest/AAPL?days=365"
```

## 🎯 Signal Guide

| Signal | Emoji | Meaning |
|--------|-------|---------|
| Strong Buy | 🚀 | High confidence buy |
| Buy | 📈 | Clear buy signal |
| Weak Buy | ↗️ | Slight bullish |
| Neutral | ➡️ | Hold/Wait |
| Weak Sell | ↘️ | Slight bearish |
| Sell | 📉 | Clear sell signal |
| Strong Sell | ⚠️ | High confidence sell |

## 🛠️ Common Commands

### Start/Stop Services
```bash
./start-all.sh          # Start everything
./stop-all.sh           # Stop everything

# Or manually:
cargo run --release --bin api-server
cargo run --release --bin discord-bot
cd frontend && ./start.sh
```

### Build
```bash
cargo build --release        # Build all
cargo build -p api-server   # Build API only
cargo build -p discord-bot  # Build bot only
```

### Logs
```bash
tail -f api-server.log
tail -f dashboard.log
```

### Docker
```bash
docker-compose up -d        # Start Redis
docker-compose down         # Stop Redis
docker ps                   # Check status
```

## 📁 Project Structure

```
invest-iq/
├── crates/               # Rust backend
│   ├── api-server/      # REST API (port 3000)
│   ├── discord-bot/     # Discord integration
│   └── [analysis engines]
├── frontend/            # Dash dashboard (port 8050)
├── .env                 # Your configuration
├── start-all.sh         # Start everything
└── stop-all.sh          # Stop everything
```

## 🔧 Configuration Files

| File | Purpose |
|------|---------|
| `.env` | Your secrets (API keys) |
| `.env.example` | Full configuration template |
| `.env.minimal` | Minimal configuration |
| `Cargo.toml` | Rust dependencies |
| `frontend/requirements.txt` | Python dependencies |

## 📈 Analysis Types

### Technical (30% weight)
- RSI, MACD, Moving Averages
- Bollinger Bands, Stochastic
- Candlestick patterns

### Fundamental (35% weight)
- P/E Ratio, ROE, Margins
- Debt ratios, Liquidity
- Cash flow analysis

### Quantitative (25% weight)
- Sharpe Ratio, Volatility
- Max Drawdown, Beta
- Value at Risk (VaR)

### Sentiment (10% weight)
- News analysis
- Positive/Negative breakdown
- Recency-weighted

## 🐛 Troubleshooting

### Can't connect to API
```bash
# Check if running
curl http://localhost:3000/health

# Start it
cargo run --release --bin api-server
```

### Discord bot not responding
1. Check Message Content Intent is enabled
2. Verify bot has permissions
3. Check bot is online in Discord

### Dashboard not loading
```bash
# Check if running
curl http://localhost:8050

# Start it
cd frontend && python app.py
```

### Redis connection failed
```bash
# System falls back to in-memory cache
# To use Redis:
docker-compose up -d
```

## 📚 Documentation

| File | Description |
|------|-------------|
| `README.md` | Project overview |
| `SETUP_GUIDE.md` | Detailed setup |
| `ARCHITECTURE.md` | System design |
| `QUICKSTART.md` | 5-minute guide |
| `DISCORD_BOT_GUIDE.md` | Discord bot details |
| `frontend/README.md` | Dashboard guide |
| `frontend/FEATURES.md` | Feature details |

## 💡 Tips

1. **Use Redis for production** - Better performance
2. **Increase cache TTL** - Reduce API calls
3. **Try popular stocks first** - AAPL, MSFT, GOOGL
4. **Check confidence scores** - Higher = more reliable
5. **Combine multiple signals** - Don't rely on just one

## ⚠️ Limits

### Free Polygon Tier
- 5 API calls per minute
- 15-minute delayed data
- Historical data available

### Solutions
- Enable caching (default 5 min)
- Upgrade Polygon plan
- Use less frequent requests

## 🆘 Getting Help

1. Check logs: `tail -f *.log`
2. Read docs in `/docs`
3. Check GitHub Issues
4. Enable debug: `RUST_LOG=debug`

## 📝 Quick Examples

### Analyze Apple
```bash
# Dashboard: Enter AAPL, click Analyze
# Discord: !iq analyze AAPL
# API: curl http://localhost:3000/api/analyze/AAPL
```

### Compare Stocks
```bash
# Dashboard: Analyze AAPL, MSFT, GOOGL one by one
# Discord:
#   !iq analyze AAPL
#   !iq analyze MSFT
#   !iq analyze GOOGL
```

### Quick Chart Check
```bash
# Discord: !iq chart TSLA
# Dashboard: Use chart command
```

---

**Keep this reference handy!** 🚀

For detailed guides, see the full documentation in the repository.
