# 🤖 Autonomous Trading Agent - Setup Guide

## System Overview

You now have a **fully autonomous trading system** that runs 24/7 on your GPU machine.

### What It Does:

1. **Scans Market** every 5 minutes (100+ stocks)
2. **Generates Signals** using 5 different strategies
3. **Asks Local AI** (running on your 5090/4090) to evaluate each trade
4. **Auto-Executes** trades that meet your guardrails
5. **Manages Positions** with automatic stop losses and take profits
6. **Sends Notifications** to Discord with all trades
7. **Tracks Performance** and adjusts strategy weights

---

## 🎯 Guardrails (Conservative Settings)

The system will ONLY trade when:
- ✅ Confidence > 75%
- ✅ Historical win rate > 60%
- ✅ Max risk per trade: 2% of account ($500 max)
- ✅ Portfolio risk < 10%
- ✅ AI approves the trade

---

## 📋 Setup Instructions (GPU Machine)

### Step 1: Install Ollama (Local LLM Server)

```bash
# On your Windows/Linux GPU machine
curl -fsSL https://ollama.com/install.sh | sh

# Pull the model (70B for best performance, 13B for faster)
ollama pull llama3.1:70b

# Or use Mistral Large
ollama pull mistral-large

# Verify it's running
curl http://localhost:11434/api/tags
```

### Step 2: Copy Project to GPU Machine

```bash
# From your Mac (this dev machine)
cd /Users/timmy/workspace/public-projects/invest-iq
tar -czf invest-iq.tar.gz .

# SCP to your GPU machine (replace with your details)
scp invest-iq.tar.gz user@gpu-machine:/path/to/trading/

# On GPU machine
cd /path/to/trading
tar -xzf invest-iq.tar.gz
```

### Step 3: Install Rust (if not installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Step 4: Configure Environment

```bash
cd /path/to/trading/invest-iq

# Copy the example
cp .env.example .env.trading

# Edit with your settings
nano .env.trading
```

Add these to `.env.trading`:

```bash
# Trading Agent Configuration
TRADING_ENABLED=true
PAPER_TRADING=true              # Start with paper trading!
SCAN_INTERVAL=300               # 5 minutes (300 seconds)

# Conservative Risk Settings
MAX_RISK_PER_TRADE=2.0         # 2% per trade
MAX_POSITION_SIZE=500.0        # $500 max per position
MAX_PORTFOLIO_RISK=10.0        # 10% total portfolio risk
MIN_CONFIDENCE=0.75            # 75% minimum confidence
MIN_WIN_RATE=0.60              # 60% historical win rate required

# Watchlist (customize these)
WATCHLIST=AAPL,MSFT,GOOGL,AMZN,NVDA,TSLA,META,AMD,NFLX,SPY

# Local LLM
LLM_ENDPOINT=http://localhost:11434
LLM_MODEL=llama3.1:70b

# APIs (from your .env)
POLYGON_API_KEY=your_key_here
ALPACA_API_KEY=your_key_here
ALPACA_SECRET_KEY=your_secret_here
ALPACA_BASE_URL=https://paper-api.alpaca.markets

# Discord Webhook (for notifications)
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_WEBHOOK_URL

# Database
DATABASE_PATH=./portfolio.db
```

### Step 5: Set Up Discord Webhook (Optional but Recommended)

1. Go to your Discord server
2. Server Settings → Integrations → Webhooks
3. Create New Webhook
4. Copy webhook URL
5. Add to `.env.trading` as `DISCORD_WEBHOOK_URL`

### Step 6: Build and Run

```bash
# Build the trading agent
cargo build --release --bin trading-agent

# Test run (will scan once and exit if no signals)
cargo run --release --bin trading-agent

# Run in background with logging
nohup cargo run --release --bin trading-agent > trading.log 2>&1 &

# Check it's running
ps aux | grep trading-agent

# Watch logs
tail -f trading.log
```

---

## 🚀 Running as a Service (Recommended)

### Systemd Service (Linux)

Create `/etc/systemd/system/trading-agent.service`:

```ini
[Unit]
Description=InvestIQ Autonomous Trading Agent
After=network.target

[Service]
Type=simple
User=youruser
WorkingDirectory=/path/to/invest-iq
EnvironmentFile=/path/to/invest-iq/.env.trading
ExecStart=/path/to/invest-iq/target/release/trading-agent
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable trading-agent
sudo systemctl start trading-agent

# Check status
sudo systemctl status trading-agent

# View logs
sudo journalctl -u trading-agent -f
```

### Windows Service

Use `nssm` (Non-Sucking Service Manager):

```powershell
# Download nssm
choco install nssm

# Install service
nssm install TradingAgent "C:\path\to\invest-iq\target\release\trading-agent.exe"
nssm set TradingAgent AppDirectory "C:\path\to\invest-iq"
nssm set TradingAgent AppEnvironmentExtra :set "TRADING_ENABLED=true"

# Start service
nssm start TradingAgent

# Check status
nssm status TradingAgent
```

---

## 📊 Monitoring

### Discord Notifications

You'll receive messages like:

```
🚀 Trading Agent Started
Autonomous trading mode activated with conservative guardrails.

🎯 BUY AAPL
15 shares @ $178.50
Confidence: 82.3%
Strategy: momentum_breakout
AI Reasoning: Strong momentum with RSI confirming uptrend. Risk/reward favorable.

🛑 STOP LOSS TRIGGERED: TSLA
Exited at $245.30
P/L: -$87.50

📊 Daily Trading Report
P/L: +$432.15 (+0.43%)
Trades: 8
Win Rate: 62.5%
Account Balance: $100,432.15
```

### Check Logs

```bash
# Real-time logs
tail -f trading.log

# Or with systemd
sudo journalctl -u trading-agent -f
```

### Web Dashboard

The trading dashboard still works:

```bash
cd frontend
python3 trading_dashboard.py

# Open http://localhost:8052
```

You'll see all trades the agent executed automatically.

---

## 🛡️ Safety Features

### Automatic Shutoff Triggers:

1. **Account drops 10%** → Stops trading, sends alert
2. **3 consecutive losses** → Reduces position size by 50%
3. **Win rate drops below 50%** → Disables underperforming strategies
4. **Market crash detected** (SPY -5%+) → Stops all trading

### Manual Override:

To stop trading immediately:

```bash
# Method 1: Stop the service
sudo systemctl stop trading-agent

# Method 2: Disable in config
# Edit .env.trading
TRADING_ENABLED=false

# Restart service
sudo systemctl restart trading-agent
```

---

## 📈 Performance Tuning

### After 2 Weeks of Paper Trading:

1. **Check Win Rate**
   ```bash
   curl http://localhost:3000/api/analytics/overview
   ```

2. **If win rate > 60%**: Continue as-is
3. **If win rate 50-60%**: Increase `MIN_CONFIDENCE` to 0.80
4. **If win rate < 50%**: Disable underperforming strategies

### Strategy Weights (in config.rs):

```rust
momentum_weight: 0.40,        // Increase if momentum works well
mean_reversion_weight: 0.25,  // Increase if reversions work
breakout_weight: 0.20,
sentiment_weight: 0.10,
high_risk_weight: 0.05,       // Reduce if too risky
```

---

## 🎯 Going Live (Real Money)

**ONLY after 60+ days of successful paper trading:**

1. **Verify Metrics:**
   - Win rate > 60%
   - Profit factor > 1.5
   - Max drawdown < 10%
   - Consistent performance (not lucky streak)

2. **Switch to Real Trading:**
   ```bash
   # In .env.trading
   PAPER_TRADING=false
   ALPACA_BASE_URL=https://api.alpaca.markets

   # Start with small account ($1000-5000)
   # Gradually increase after proving it works
   ```

3. **Monitor Closely:**
   - Check Discord notifications daily
   - Review all trades
   - Be ready to shut down if needed

---

## ⚠️ Important Disclaimers

- **Start with PAPER TRADING** (fake money)
- **Test for 60+ days** before real money
- **Markets are unpredictable** - no guarantee of profit
- **Past performance ≠ future results**
- **You can lose money** - only risk what you can afford
- **AI is not perfect** - always monitor the system
- **This is experimental** - use at your own risk

---

## 🆘 Troubleshooting

**Agent not starting:**
- Check Ollama is running: `curl http://localhost:11434/api/tags`
- Check environment variables: `cat .env.trading`
- Check logs: `tail -f trading.log`

**No trades executing:**
- Check `TRADING_ENABLED=true`
- Lower `MIN_CONFIDENCE` temporarily to test
- Check if signals are being generated in logs

**Ollama out of memory:**
- Use smaller model: `ollama pull llama3.1:13b`
- Or use Mistral 7B: `ollama pull mistral:7b`

**API rate limits:**
- Polygon: Max 5 requests/min on free tier
- Increase `SCAN_INTERVAL` to 600 (10 minutes)

---

## 📚 Next Steps

1. ✅ Install Ollama on GPU machine
2. ✅ Copy project and configure
3. ✅ Test with paper trading
4. ✅ Monitor for 60 days
5. ✅ Validate performance
6. ⚠️ Consider real money (if metrics are good)

---

## 🎉 You're All Set!

Your autonomous trading agent is ready to deploy. It will:
- Scan markets 24/7
- Make intelligent decisions using AI
- Execute trades automatically
- Manage risk conservatively
- Notify you of everything

**Remember:** Start with paper trading and monitor closely!
