# 🚀 Trading Quick Start Guide

## You Now Have TWO Ways to Trade!

Both options are **fully implemented** and ready to use right now.

---

## Option 1: Visual Web Dashboard (CLICK BUTTONS!)

### What You Get:
- 💰 Account balance banner at top
- 🔔 Action inbox with **Execute Trade** buttons
- 📊 Real-time portfolio display
- 📜 Recent trades history
- ✅ Confirmation dialogs before executing
- 🔔 Success/error notifications
- 🔄 Auto-refresh every 30 seconds

### Start It:

**Terminal 1 - API Server:**
```bash
cd /Users/timmy/workspace/public-projects/invest-iq
cargo run --release --bin api-server
```

Wait for:
```
✅ Alpaca broker connected (Paper Trading Mode)
Server listening on 0.0.0.0:3000
```

**Terminal 2 - Web Dashboard:**
```bash
cd /Users/timmy/workspace/public-projects/invest-iq/frontend

# Set your API key (get it from .env file)
export API_KEY=your_api_key_here

# Start dashboard
python3 trading_dashboard.py
```

**Open Browser:**
```
http://localhost:8052
```

### How to Trade:
1. **See your balance** at the top
2. **Scroll to Action Inbox** - see all trading signals
3. **Click "Execute BUY/SELL" button** on any signal
4. **Confirmation dialog appears** - enter number of shares
5. **Click "Execute Trade"**
6. **Success notification appears**
7. **Trade logs automatically**, portfolio updates automatically
8. **Check Alpaca dashboard** to see your paper trade!

---

## Option 2: Command-Line Script (NO GUI NEEDED!)

### What You Get:
- 💰 Shows account balance
- 📊 Shows current portfolio
- 🔔 Shows all action items one-by-one
- ✅ Prompts you Y/N to execute each trade
- 📝 Auto-logs trades and updates portfolio
- 🎯 Perfect for quick trading sessions

### Start It:

**Terminal 1 - API Server:**
```bash
cd /Users/timmy/workspace/public-projects/invest-iq
cargo run --release --bin api-server
```

**Terminal 2 - Trading Script:**
```bash
cd /Users/timmy/workspace/public-projects/invest-iq/frontend

# Set your API key
export API_KEY=your_api_key_here

# Run script
python3 click_to_trade.py
```

### How to Trade:
The script will:
1. Show your account balance
2. Show your current portfolio
3. Show each trading signal
4. Ask you: "Execute BUY for AAPL? (y/n or number of shares):"
5. You type: `y` (uses 10 shares default) or `5` (buys 5 shares)
6. Trade executes immediately
7. Shows confirmation
8. Moves to next signal

**Example Session:**
```
💰 InvestIQ Click-to-Trade
============================================================

📊 Account Balance:
------------------------------------------------------------
   💵 Cash: $100,000.00
   💰 Buying Power: $100,000.00
   📈 Portfolio Value: $100,000.00
------------------------------------------------------------

🔔 Action Inbox (3 items):
============================================================

1. AAPL - Strong Buy Signal
   Action: BUY
   Confidence: 87%
   Current Price: $178.50
   Target Price: $195.00
   Apple showing strong momentum breakout

   --------------------------------------------------
   Execute BUY for AAPL? (y/n or number of shares): 10

   Executing BUY 10 shares of AAPL...
   ✅ Trade executed successfully!
      Order ID: abc123...
      Status: filled
      Fill Price: $178.50

2. NVDA - Take Profit Opportunity
   Action: SELL
   Confidence: 75%
   Current Price: $485.20
   ✓ Already in portfolio
   Target reached, consider taking profits

   --------------------------------------------------
   Execute SELL for NVDA? (y/n or number of shares): n
   ⏭️  Skipped

============================================================
✅ Session complete: 1 trade(s) executed

ℹ️  Trades have been auto-logged and portfolio updated
   Check Alpaca dashboard: https://app.alpaca.markets/paper/dashboard
============================================================
```

---

## Which Option Should You Use?

### Use Web Dashboard When:
- ✅ You want a visual interface
- ✅ You want to see everything at once
- ✅ You're comfortable leaving a browser tab open
- ✅ You want real-time auto-refresh

### Use CLI Script When:
- ✅ You want quick trading sessions
- ✅ You prefer terminal/command-line
- ✅ You want to process signals one-by-one
- ✅ You don't want to open a browser

### Use BOTH When:
- ✅ You want the best of both worlds!
- ✅ Use CLI for quick morning trades
- ✅ Use dashboard to monitor throughout the day

---

## What Happens After You Execute a Trade?

### Automatic Actions:
1. ✅ Trade submits to Alpaca (paper trading - fake money)
2. ✅ Order fills at current market price
3. ✅ Trade automatically logs to database
4. ✅ Portfolio automatically updates
5. ✅ P&L automatically calculates

### Where to See Results:

**Alpaca Dashboard:**
```
https://app.alpaca.markets/paper/dashboard
```

**Your Database:**
```bash
sqlite3 portfolio.db "SELECT * FROM trades ORDER BY created_at DESC LIMIT 5;"
```

**Your Portfolio API:**
```bash
curl -H "X-API-Key: your_key" http://localhost:3000/api/portfolio
```

**Recent Trades API:**
```bash
curl -H "X-API-Key: your_key" http://localhost:3000/api/trades
```

---

## Complete Trading Workflow

### Morning Routine (5 minutes):

**Option A - Web Dashboard:**
1. Start API server (Terminal 1)
2. Start web dashboard (Terminal 2)
3. Open browser to http://localhost:8052
4. Review action inbox
5. Click execute buttons on high-confidence signals
6. Confirm trades
7. Done!

**Option B - CLI Script:**
1. Start API server (Terminal 1)
2. Run `python3 click_to_trade.py` (Terminal 2)
3. Review each signal as it appears
4. Type `y` or number of shares to execute
5. Type `n` to skip
6. Done!

### Evening Routine (2 minutes):

**Check Performance:**
```bash
# Option 1: Web dashboard
# Just open http://localhost:8052 - see portfolio section

# Option 2: API call
curl -H "X-API-Key: your_key" http://localhost:3000/api/trades/performance
```

---

## Environment Setup

### Required Environment Variables:

**In .env file (for API server):**
```bash
ALPACA_API_KEY=PKQJUVHFMUTBAWDDL7EWSIGHVZ
ALPACA_SECRET_KEY=57hjT44a5yWdEQ5iYg19UTcaKHvAMTS7SL8N3R6XhqcW
ALPACA_BASE_URL=https://paper-api.alpaca.markets
```

**In shell (for Python scripts):**
```bash
export API_KEY=your_api_key_here  # Get from .env file
```

**Set Once Per Session:**
```bash
# Add to your ~/.bashrc or ~/.zshrc for persistence
echo 'export API_KEY=your_api_key_here' >> ~/.bashrc
source ~/.bashrc
```

---

## Troubleshooting

### "Unable to connect to API server"
**Solution:**
```bash
# Make sure API server is running
cd /Users/timmy/workspace/public-projects/invest-iq
cargo run --release --bin api-server
```

### "API_KEY environment variable not set"
**Solution:**
```bash
export API_KEY=your_api_key_here
```

### "Error fetching account"
**Solution:**
Check that Alpaca credentials in `.env` are correct:
```bash
cat .env | grep ALPACA
```

### Web dashboard not loading
**Solution:**
```bash
# Install dependencies
pip3 install dash dash-bootstrap-components plotly requests

# Try again
python3 trading_dashboard.py
```

### CLI script not working
**Solution:**
```bash
# Install requests
pip3 install requests

# Check API key is set
echo $API_KEY

# Try again
python3 click_to_trade.py
```

---

## Safety Features

### Paper Trading = No Risk:
- ✅ Using fake $100,000
- ✅ Real market prices
- ✅ Real order flow
- ✅ Zero cost to you
- ✅ Learn safely

### Confirmation Before Execution:
- ✅ Web dashboard shows confirmation dialog
- ✅ CLI script asks Y/N for each trade
- ✅ You control every trade
- ✅ No auto-trading (you must click/confirm)

### Auto-Logging Protection:
- ✅ Every trade logs to database
- ✅ Full audit trail
- ✅ Can review all trades anytime
- ✅ Performance metrics tracked

---

## Next Steps

### Start Trading NOW:
1. Choose your option (Web dashboard or CLI script)
2. Follow the "Start It" instructions above
3. Execute your first paper trade
4. Check results in Alpaca dashboard

### After Your First Trade:
1. ✅ Verify it appears in Alpaca dashboard
2. ✅ Verify it logged to database
3. ✅ Verify portfolio updated
4. ✅ Check P&L calculation

### Build Your Strategy:
1. 📊 Trade daily for 6-8 weeks
2. 📈 Track your performance
3. 🎯 Refine which signals you follow
4. 💰 When profitable, consider real money (carefully!)

---

## Files Reference

### Scripts:
- **Web Dashboard:** `frontend/trading_dashboard.py`
- **CLI Script:** `frontend/click_to_trade.py`

### Backend:
- **API Server:** `cargo run --release --bin api-server`
- **Database:** `portfolio.db`
- **Config:** `.env`

### Documentation:
- **This Guide:** `TRADING_QUICK_START.md`
- **Complete System:** `SYSTEM_COMPLETE.md`
- **Portfolio Guide:** `PORTFOLIO_GUIDE.md`

---

## Summary

### ✅ What's Working:
- Backend API: 100% complete
- Alpaca integration: Fully functional
- Web dashboard: Fully functional with click buttons
- CLI script: Fully functional
- Auto-logging: Active
- Portfolio tracking: Active
- Paper trading: $100k ready

### 🎯 What You Do:
1. Start the systems
2. See trading signals
3. Click/confirm to execute
4. Watch results

### 💰 What You Get:
- Safe practice trading
- Real market experience
- Performance tracking
- No risk learning

---

## 🚀 Ready to Trade!

**Choose your weapon:**
- 🖥️ Web Dashboard: http://localhost:8052 (after starting)
- 💻 CLI Script: `python3 click_to_trade.py`

**Both are ready RIGHT NOW!**

Happy paper trading! 📈💰
