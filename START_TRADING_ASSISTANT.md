# 🚀 Quick Start: Trading Assistant

## What You're About to Use

InvestIQ is now a **complete trading assistant** that:
- ✅ Tracks your portfolio in real-time
- ✅ Logs all your trades
- ✅ Calculates your profits automatically
- ✅ Gives you daily actionable signals ("Buy AAPL now!")
- ✅ Shows your performance metrics

## 5-Minute Setup

### 1. Configure API Keys

Edit `.env` file:

```bash
# Required
POLYGON_API_KEY=your_polygon_api_key_here
API_KEYS=generate_with_openssl_rand_hex_32

# Optional
DATABASE_URL=sqlite:portfolio.db
```

Generate API key:
```bash
openssl rand -hex 32
```

### 2. Update Frontend API Key

Edit `frontend/portfolio_app.py`:

```python
API_KEY = "paste_your_generated_key_here"  # Line 17
```

### 3. Start the System

**Terminal 1 - API Server:**
```bash
cargo run --release --bin api-server
```

**Terminal 2 - Portfolio Dashboard:**
```bash
cd frontend
python portfolio_app.py
```

### 4. Open Dashboard

Go to: http://localhost:8052

---

## First Steps

### Add Your First Position

1. Go to **"My Portfolio"** tab
2. Scroll to "Add New Position"
3. Enter:
   - Symbol: AAPL
   - Shares: 10
   - Entry Price: 150.00
   - Date: When you bought it
   - Notes: "Test position"
4. Click "➕ Add Position"
5. Click "🔄 Refresh Portfolio" to see live P&L

### Check Action Inbox

1. Go to **"Action Inbox"** tab
2. See today's trading signals
3. Each action shows:
   - What to do (Buy/Sell)
   - Confidence level
   - Target price
   - Stop loss
   - Whether you own the stock

### Log a Trade

1. Go to **"Trade Log"** tab
2. Click "Log New Trade"
3. Fill in details:
   - Symbol: AAPL
   - Action: Buy or Sell
   - Shares: 10
   - Price: 150.00
   - Date: Trade date
   - Commission: 0 (if using Robinhood)
4. Click "📝 Log Trade"

Your portfolio updates automatically!

### View Performance

Scroll down in **"Trade Log"** tab to see:
- Total trades
- Win rate
- Total realized P&L
- Average win/loss

---

## Daily Workflow

### Morning (5 min)
1. Open http://localhost:8052
2. Check **Action Inbox**
3. Review high-confidence signals (>80%)
4. Prioritize stocks you already own

### When You Trade
1. Execute the trade in your broker app
2. Immediately log it in **Trade Log**
3. System updates portfolio automatically

### Evening (2 min)
1. Check **My Portfolio** for unrealized P&L
2. Review any stop loss warnings
3. Mark completed actions as done

### Weekly Review (10 min)
1. Check **Performance Metrics**
2. Review win rate (target: >55%)
3. Analyze which signals worked
4. Adjust strategy if needed

---

## What Each Tab Does

### 🔔 Action Inbox
**"Your personal trading assistant"**
- Shows prioritized trading signals
- Tells you exactly what to do today
- Highlights stocks you own
- Marks urgent actions (stop losses, take profits)

**When to use:** Every morning to plan your trades

### 📊 My Portfolio
**"Track your money in real-time"**
- Live portfolio value
- Individual position P&L
- Total return percentage
- Cost basis tracking

**When to use:**
- After logging trades
- To check current profits
- Before making new trades

### 📝 Trade Log
**"Your trading diary"**
- Complete trade history
- Performance metrics
- Win rate tracking
- Realized P&L

**When to use:**
- After EVERY trade (buy or sell)
- Monthly performance reviews
- Tax time

### 👀 Watchlist
**"Stocks you're interested in"**
- Monitor potential trades
- Add notes on why watching
- Quick access to analysis

**When to use:**
- When you find interesting stocks
- Before earnings reports
- For stocks mentioned in news

---

## Example: Making Your First $100

### Day 1: Buy
1. **Action Inbox shows:** "🚀 Buy AAPL at $150 (85% confidence)"
2. **You buy:** 10 shares at $150 in your broker
3. **Log trade:** Trade Log → Buy 10 AAPL @ $150
4. **Portfolio shows:** AAPL: $1,500 cost basis

### Day 5: Stock Up
1. **Portfolio shows:** AAPL → $160 (+$100, +6.7%)
2. **Action Inbox shows:** "📈 Take Profit: AAPL target hit"
3. **Decision:** Sell to lock in $100 profit

### Day 5: Sell
1. **You sell:** 10 shares at $160 in your broker
2. **Log trade:** Trade Log → Sell 10 AAPL @ $160
3. **Performance shows:**
   - Realized P&L: +$100
   - Win rate: 100% (1 winner)
   - Average win: $100

**You made $100 by following the system!** 🎉

---

## Pro Tips for Success

### 1. Always Log Trades Immediately
- Don't wait until end of day
- Details fade from memory
- Accurate records = accurate performance metrics

### 2. Follow High-Confidence Signals
- >80% confidence: Strong consideration
- 60-80%: Good opportunities
- <60%: Use caution or paper trade first

### 3. Use Stop Losses
- System suggests them for each trade
- Limits losses to 5% or less
- Protects your capital

### 4. Take Profits at Targets
- Don't be greedy
- Lock in gains when targets hit
- Can always buy back later

### 5. Review Performance Monthly
- Track win rate (target: >55%)
- Check if avg win > 2x avg loss
- Adjust if numbers declining

### 6. Start Small
- Begin with $500-1000 per trade
- Test the system with real money
- Scale up as you gain confidence

---

## Common Questions

**Q: Does this trade automatically?**
A: No. You make all trades manually in your broker. This system just tells you what to do and tracks results.

**Q: How accurate are the signals?**
A: Backtests show 55-65% win rates typically. Results vary by market conditions.

**Q: Can I lose money?**
A: Yes. All trading involves risk. Only trade money you can afford to lose. Always use stop losses.

**Q: Do I need a paid broker account?**
A: No. Works with any broker (Robinhood, Fidelity, etc.). System just tracks your trades.

**Q: What if I make a mistake logging a trade?**
A: Go to Trade Log, find the trade, and delete or edit it. All data is in local database.

**Q: Can I track multiple portfolios?**
A: Yes. Use different database files (see advanced docs).

---

## Troubleshooting

### Dashboard won't load
```bash
# Check API server is running
curl http://localhost:3000/health

# Should return: {"success":true,"data":{"status":"healthy"}}
```

### "Authentication failed"
- Check API_KEY in `portfolio_app.py` matches one in `.env` API_KEYS
- Make sure to include X-API-Key header

### Prices not updating
- Free tier has 15-min delay
- Click "Refresh Portfolio" button
- Upgrade Polygon.io for real-time

### "Portfolio manager not initialized"
- Check DATABASE_URL in `.env`
- Verify portfolio.db file created
- Check API server logs for errors

---

## Success Metrics

Track these to know if system is working:

### Week 1:
- [ ] Successfully logged 3+ trades
- [ ] Portfolio tracking working
- [ ] Can see P&L for each position
- [ ] Action Inbox showing signals

### Month 1:
- [ ] Win rate >50%
- [ ] At least one profitable trade
- [ ] Comfortable with daily workflow
- [ ] Performance metrics make sense

### Month 3:
- [ ] Win rate >55%
- [ ] Average win > average loss
- [ ] Consistent use of system
- [ ] Positive total P&L

---

## Next Steps

Once comfortable with basics:

1. **Read full guide:** See `PORTFOLIO_GUIDE.md`
2. **Explore API:** See all endpoints and features
3. **Set up Discord alerts:** Get notifications
4. **Add cron jobs:** Auto-save daily snapshots
5. **Export data:** Analyze in Excel/Python

---

## 🎯 Your Goal

**Make consistent, data-driven trades and track results.**

This system gives you:
- ✅ Clear signals on what to buy/sell
- ✅ Automatic P&L tracking
- ✅ Performance metrics to improve
- ✅ Historical record for taxes

**Now go make some money!** 💰📈

---

Questions? Check `PORTFOLIO_GUIDE.md` for detailed docs.

Good luck and trade wisely! 🚀
