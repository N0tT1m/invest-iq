# 🚀 InvestIQ - START HERE

## ✅ System Configured and Ready!

All dependencies installed and configuration complete. Your trading assistant is ready to test.

---

## 📋 Current Status

✅ **API Server** - Compiled successfully
✅ **Python Dependencies** - Installed in virtual environment
✅ **Database** - Initialized (portfolio.db)
✅ **Alpaca** - Connected to paper trading account
✅ **API Keys** - Configured
✅ **Dashboard** - Ready to run

---

## 🎯 How to Start Testing (2 Steps)

### Step 1: Start API Server

```bash
cd /Users/timmy/workspace/public-projects/invest-iq
cargo run --release --bin api-server
```

**Wait for these messages:**
```
✅ Alpaca broker connected (Paper Trading Mode)
✅ Portfolio database initialized
✅ Risk manager initialized
🚀 API Server starting on 0.0.0.0:3000
```

### Step 2: Start Trading Dashboard (New Terminal)

```bash
cd /Users/timmy/workspace/public-projects/invest-iq/frontend
source ../venv/bin/activate
export API_KEY=79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219
python3 trading_dashboard.py
```

**Then open:** http://localhost:8052

---

## 💻 What You'll See

The dashboard has **4 tabs:**

### 1. **Account Overview**
- Your Alpaca paper trading balance ($100k fake money)
- Current buying power
- Portfolio value

### 2. **Get Signals**
- Click "Get Signals" button
- Enter a stock symbol (e.g., AAPL, TSLA, NVDA)
- See buy/sell recommendations with confidence scores
- Risk-approved position sizes
- Stop loss and take profit levels

### 3. **Execute Trades**
- Review the signal
- Click "Execute Buy" or "Execute Sell"
- Trade executes on Alpaca paper account
- See confirmation

### 4. **Portfolio & Performance**
- View all your positions
- Real-time P/L
- Trade history
- Performance metrics

---

## 🧪 Test Workflow

### First Test (5 minutes):

1. **Start both services** (API + Dashboard)
2. **Click "Get Signals"** tab
3. **Enter "AAPL"** and submit
4. **Review the analysis:**
   - Technical signal (Buy/Sell/Hold)
   - Confidence score (0-100%)
   - Entry price
   - Stop loss price
   - Take profit target
5. **Click "Execute Buy"** if signal is strong
6. **Check "Portfolio"** tab to see your position

### Today's Goal:
- [ ] Execute 2-3 paper trades
- [ ] Verify trades show up in Alpaca
- [ ] Check portfolio tracking works
- [ ] Review signal quality

---

## 📊 Testing Checklist

### Week 1:
- [ ] Test 5-10 different stocks
- [ ] Execute trades on high-confidence signals (>70%)
- [ ] Verify portfolio updates correctly
- [ ] Check P/L calculations

### Weeks 2-8:
- [ ] Run daily, check signals
- [ ] Track win rate (goal: >55%)
- [ ] Monitor max drawdown (goal: <10%)
- [ ] Validate the system actually works

---

## 🔑 Important Info

**Your API Key:**
```
79f638783cd54238a872adb66727f9ccfa03718dcec1be92ff84362e6f6f2219
```

**Alpaca Account:**
- Type: Paper Trading (fake money)
- Starting Balance: $100,000
- URL: https://paper-api.alpaca.markets

**Ports:**
- API Server: http://localhost:3000
- Trading Dashboard: http://localhost:8052

---

## 🐛 Troubleshooting

**API Server won't start:**
- Check Redis is running: `redis-cli ping` (should return PONG)
- If Redis not installed: `brew install redis && brew services start redis`

**Dashboard shows "Unable to connect":**
- Make sure API server is running first
- Check API_KEY environment variable is set
- Verify .env file has all required keys

**Trades not executing:**
- Check Alpaca paper trading account status
- Verify ALPACA_API_KEY and ALPACA_SECRET_KEY in .env
- Look at API server logs for errors

**"No signals" or errors:**
- Wait 1 minute between requests (Polygon API rate limit)
- Try a different stock symbol
- Check API server logs

---

## 📚 Next Steps

After testing works:

1. **Read the docs:**
   - `MONEY_MACHINE_COMPLETE.md` - Full system overview
   - `TRADING_QUICK_START.md` - Daily workflows
   - `PORTFOLIO_GUIDE.md` - Portfolio features

2. **Run backtests:**
   ```bash
   curl -H "X-API-Key: YOUR_KEY" \
     -X POST http://localhost:3000/api/backtest/run \
     -d '{"symbol": "AAPL", "days": 365}'
   ```

3. **Track performance:**
   - Use the system daily for 60 days
   - Monitor win rate
   - Validate signals are profitable

---

## ⚠️ Remember

- This is **PAPER TRADING** (fake money)
- Test thoroughly before considering real money
- Past performance ≠ future results
- No system wins 100% of the time
- Markets are unpredictable

---

## 🎉 You're Ready!

Start the two services and begin testing. The system is configured and ready to go.

**Questions?** Check the docs in the project root.

**Happy (paper) trading!** 📈💰
