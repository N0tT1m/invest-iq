# InvestIQ Portfolio Manager & Trading Assistant Guide

## 🎉 NEW FEATURES

Your InvestIQ system is now a **complete trading assistant** that helps you track profits, manage positions, and get actionable trading signals!

## 🚀 What's New

### 1. **Action Inbox** - Your Trading Assistant
Get daily actionable insights telling you exactly what to do:
- 🚀 **Buy Signals** - "Buy NVDA at $850 (87% confidence)"
- 📈 **Take Profit** - "AAPL up 18.7% - consider selling"
- ⚠️ **Stop Loss Warnings** - "TSLA approaching stop loss"
- 📌 **Watch Alerts** - Stocks showing interesting patterns

### 2. **Portfolio Tracker**
See your money in real-time:
- Track all your stock positions
- Live P&L calculations
- Total portfolio value
- Individual stock performance
- Cost basis tracking

### 3. **Trade Logger**
Keep a complete record:
- Log every buy/sell trade
- Track commissions
- Calculate realized profits
- Performance metrics (win rate, avg win/loss)
- Trade history

### 4. **Watchlist**
Monitor stocks you're interested in:
- Add stocks to watch
- Add notes on why you're watching
- Quick access to analysis

---

## 📋 Quick Start

### Step 1: Start the API Server

```bash
# Make sure you have your API keys in .env
cargo run --release --bin api-server
```

The server will start on `http://localhost:3000`

### Step 2: Start the Portfolio Dashboard

```bash
cd frontend
python portfolio_app.py
```

The portfolio manager will run on `http://localhost:8052`

**IMPORTANT:** Open `portfolio_app.py` and update `API_KEY` with your actual API key!

### Step 3: Use the System

Open http://localhost:8052 in your browser and you'll see:
- **Action Inbox** tab - Your daily trading signals
- **My Portfolio** tab - Track your positions
- **Trade Log** tab - Record all trades
- **Watchlist** tab - Monitor interesting stocks

---

## 💡 How to Use This System to Make Money

### Daily Workflow:

**Morning Routine (5 minutes):**
1. Open the **Action Inbox** tab
2. Review new signals (sorted by priority)
3. Check which signals are for stocks you own
4. Make trading decisions

**When You Make a Trade:**
1. Go to **Trade Log** tab
2. Click "Log New Trade"
3. Fill in details (symbol, buy/sell, shares, price, date)
4. Click "Log Trade"
5. Your portfolio updates automatically!

**Checking Your Performance:**
1. Go to **My Portfolio** tab
2. See total P&L and portfolio value
3. Click "Refresh Portfolio" for latest prices
4. Check individual position performance

**End of Month:**
1. Go to **Trade Log** tab
2. View "Performance Metrics" card
3. Review win rate, avg win/loss
4. Analyze what worked and what didn't

---

## 📊 Example Usage

### Example 1: Buy a Stock

1. **Action Inbox shows:**
   ```
   🚀 StrongBuy Signal: NVDA
   Buy NVDA at $850-855 (87% confidence)
   Target: $920 (+8%)
   Stop Loss: $810 (-5%)
   ```

2. **You buy:**
   - 10 shares of NVDA at $852

3. **Log the trade:**
   - Trade Log → "Log New Trade"
   - Symbol: NVDA
   - Action: Buy
   - Shares: 10
   - Price: $852
   - Date: Today
   - Commission: $0 (if using Robinhood)
   - Notes: "Following InvestIQ StrongBuy signal"
   - Click "Log Trade"

4. **Track it:**
   - Go to Portfolio tab
   - Click "Refresh Portfolio"
   - See: "NVDA: 10 shares @ $852 → Now $865 = +$130 (+1.5%)"

### Example 2: Take Profit

1. **Action Inbox shows:**
   ```
   📈 TAKE PROFIT: AAPL (You own 10 shares)
   Current: $178 (+18.7% gain)
   Target hit! Consider selling 50%
   ```

2. **You sell:**
   - 5 shares (half) at $178

3. **Log the trade:**
   - Trade Log → "Log New Trade"
   - Symbol: AAPL
   - Action: Sell
   - Shares: 5
   - Price: $178
   - Click "Log Trade"

4. **See your profit:**
   - Portfolio automatically updates
   - Trade Log shows realized P&L
   - Performance Metrics updates win rate

### Example 3: Monthly Review

**At end of month, check Performance Metrics:**
```
Total Trades: 24
Win Rate: 62.5% (15 wins / 9 losses)
Total Realized P&L: +$2,347.89
Average Win: $287.43
Average Loss: -$122.56
```

**Analysis:**
- Win rate >60% = Good! System is working
- Avg Win > 2x Avg Loss = Excellent risk/reward
- Keep following high-confidence signals

---

## 🔔 Action Inbox - Priority System

Actions are sorted by priority:

**Priority 1 (Red - Urgent):**
- Strong Buy signals (>80% confidence)
- Stop loss warnings for your positions
- Take profit targets hit

**Priority 2 (Orange - Important):**
- Regular Buy signals (50-80% confidence)
- Sell signals for stocks you own

**Priority 3 (Blue - Watch):**
- Stocks showing interesting patterns
- Lower confidence signals
- Watchlist updates

### How to Handle Actions:

For each action, you can:
- **✅ Complete** - Mark as done after you execute the trade
- **❌ Ignore** - Dismiss if you decide not to follow it

---

## 📈 Understanding Your Numbers

### Portfolio Summary:
- **Total Value** - Current market value of all positions
- **Total Cost** - What you originally paid (cost basis)
- **Total P&L** - Your unrealized profit/loss (not sold yet)
- **Return %** - Percentage gain/loss

### Individual Positions:
- **Entry Price** - What you bought at
- **Current Price** - Live market price
- **Market Value** - What it's worth now (shares × current price)
- **Unrealized P&L** - Profit/loss if you sold right now

### Performance Metrics:
- **Total Trades** - Number of completed trades
- **Win Rate** - % of profitable trades
- **Total Realized P&L** - Actual profit from closed positions
- **Avg Win/Loss** - Average profit on winners vs. average loss on losers

---

## 🎯 Trading Strategy Tips

### Follow High-Confidence Signals:
- Signals >80% confidence: Seriously consider
- Signals 60-80%: Good opportunities
- Signals <60%: Use caution

### Risk Management:
- Always use stop losses (InvestIQ suggests them)
- Don't risk more than 2-5% of portfolio per trade
- Take profits at targets (don't be greedy)

### Position Sizing:
- Start with $1,000-$2,000 per position
- Don't put all money in one stock
- Keep 10-20% cash for opportunities

### Track Everything:
- Log EVERY trade (even losses)
- Review monthly performance
- Learn from winning AND losing trades

---

## 🔧 Configuration

### Environment Variables

Add to your `.env` file:

```bash
# Required
POLYGON_API_KEY=your_polygon_key

# Required for API authentication
API_KEYS=your_generated_api_key_1,your_generated_api_key_2

# Optional - Portfolio database location
DATABASE_URL=sqlite:portfolio.db

# Optional
REDIS_URL=redis://localhost:6379
ALPHA_VANTAGE_API_KEY=your_alpha_vantage_key
```

### Generate API Keys:

```bash
openssl rand -hex 32
```

Copy the output and add to `API_KEYS` in `.env` AND update `API_KEY` in `portfolio_app.py`

---

## 📡 API Endpoints

### Portfolio:
- `GET /api/portfolio` - Get portfolio summary with live prices
- `GET /api/portfolio/positions` - List all positions
- `POST /api/portfolio/positions` - Add new position
- `PUT /api/portfolio/positions/:symbol` - Update position
- `DELETE /api/portfolio/positions/:symbol` - Remove position
- `GET /api/portfolio/snapshots?days=30` - Get historical snapshots

### Trades:
- `GET /api/trades` - Get trade history
- `POST /api/trades` - Log new trade
- `GET /api/trades/:id` - Get specific trade
- `PUT /api/trades/:id` - Update trade
- `DELETE /api/trades/:id` - Delete trade
- `GET /api/trades/performance?days=30` - Get performance metrics

### Alerts:
- `GET /api/alerts` - Get active alerts
- `POST /api/alerts` - Create new alert
- `POST /api/alerts/:id/complete` - Mark alert as completed
- `POST /api/alerts/:id/ignore` - Ignore alert
- `DELETE /api/alerts/:id` - Delete alert
- `GET /api/alerts/actions` - Get actionable items (for Action Inbox)

### Watchlist:
- `GET /api/watchlist` - Get watchlist
- `POST /api/watchlist` - Add to watchlist
- `DELETE /api/watchlist/:symbol` - Remove from watchlist

---

## 🗄️ Database

The system uses SQLite to store your data in `portfolio.db`:

### Tables:
- **positions** - Current holdings
- **trades** - Trade history
- **alerts** - Signals and actions
- **watchlist** - Monitored stocks
- **portfolio_snapshots** - Daily equity curve

### Backup Your Data:

```bash
# Backup
cp portfolio.db portfolio_backup_$(date +%Y%m%d).db

# Restore
cp portfolio_backup_20250104.db portfolio.db
```

---

## 🔥 Pro Tips

### 1. Set Up Daily Snapshots

Create a cron job to save daily portfolio snapshots:

```bash
# Add to crontab
0 16 * * 1-5 curl -X POST -H "X-API-Key: your_key" http://localhost:3000/api/portfolio/snapshots
```

This runs Monday-Friday at 4pm (market close) to track your equity curve.

### 2. Export Trade History

```bash
# Get trades as JSON
curl -H "X-API-Key: your_key" http://localhost:3000/api/trades > trades.json

# Or use Python to export to CSV
import pandas as pd
import requests

response = requests.get('http://localhost:3000/api/trades',
                       headers={'X-API-Key': 'your_key'})
trades = response.json()['data']
df = pd.DataFrame(trades)
df.to_csv('trades.csv', index=False)
```

### 3. Create Custom Alerts

Use the screener + create alerts automatically:

```bash
# Get top suggestions
curl -H "X-API-Key: your_key" "http://localhost:3000/api/suggest?universe=popular&limit=5"

# Create alert for each (use script)
```

### 4. Track Multiple Portfolios

Use different database files:

```bash
# Portfolio 1 (long-term)
DATABASE_URL=sqlite:portfolio_longterm.db cargo run --release --bin api-server

# Portfolio 2 (day trading)
DATABASE_URL=sqlite:portfolio_daytrading.db cargo run --release --bin api-server
```

---

## ⚠️ Important Disclaimers

1. **Not Financial Advice** - This tool provides data and analysis, but YOU make the trading decisions.

2. **Past Performance ≠ Future Results** - Backtests and historical signals don't guarantee future profits.

3. **Risk of Loss** - Trading stocks involves risk. Only trade with money you can afford to lose.

4. **Data Accuracy** - Always verify prices before executing trades. System uses delayed data on free tier.

5. **Tax Implications** - Track your trades for tax purposes. Consult a tax professional.

6. **No Automatic Trading** - System does NOT execute trades automatically. You must manually place orders.

---

## 🐛 Troubleshooting

### "Portfolio manager not initialized"
- Check that API server started successfully
- Verify DATABASE_URL is set correctly
- Check logs: `tail -f api-server.log`

### "Error fetching portfolio"
- Verify API key is correct in frontend
- Check API server is running: `curl http://localhost:3000/health`
- Check CORS settings in `.env`

### Prices not updating
- Free tier has 15-min delayed data
- Refresh manually with "Refresh Portfolio" button
- Upgrade Polygon.io for real-time data

### Database locked error
- Only one API server instance can access database
- Stop other instances: `pkill api-server`
- Restart: `cargo run --release --bin api-server`

---

## 📚 Next Steps

### Add Discord Notifications

Edit `crates/discord-bot/src/main.rs` to send alerts:

```rust
// TODO: Add alert notification feature
// When new high-confidence alert created, send Discord DM
```

### Add Email Reports

Use cron + curl to send daily summaries:

```bash
# Daily summary at 5pm
0 17 * * 1-5 /path/to/send_daily_report.sh
```

### Mobile App

The API is REST-based, so you can build a mobile app:
- React Native
- Flutter
- Swift/Kotlin

---

## 🙏 Support

- GitHub Issues: https://github.com/your-repo/invest-iq/issues
- Documentation: This file!
- API Docs: See API Endpoints section above

---

## ✅ Success Checklist

Before you start trading:
- [ ] API server running on port 3000
- [ ] Portfolio dashboard running on port 8052
- [ ] API key configured in frontend
- [ ] Polygon.io API key working
- [ ] Can add a test position
- [ ] Can log a test trade
- [ ] Can see portfolio summary
- [ ] Action inbox showing signals

Ready to make money! 🚀💰

---

**Remember:** The goal is to make CONSISTENT, DISCIPLINED trades based on data. Track everything, review regularly, and adjust your strategy based on what the numbers tell you.

Good luck and trade wisely! 📈
