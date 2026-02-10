# 🚀 Quick Start: Paper Trading with Alpaca

## ✅ Backend Is Ready!

The API server now has full Alpaca integration. Your paper trading account is configured and ready!

## 🎯 What Works Right Now

### Test Your Setup (2 minutes)

**1. Start the API server:**
```bash
cargo run --release --bin api-server
```

You should see:
```
✅ Alpaca broker connected (Paper Trading Mode)
   Using: https://paper-api.alpaca.markets
```

**2. Test the account endpoint:**
```bash
curl -H "X-API-Key: your_key" http://localhost:3000/api/broker/account
```

You'll see your $100,000 paper trading balance!

**3. Execute a test trade (API only for now):**
```bash
curl -X POST \
  -H "X-API-Key: your_key" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"AAPL","action":"buy","shares":1,"notes":"Test trade"}' \
  http://localhost:3000/api/broker/execute
```

This will:
- ✅ Execute trade on Alpaca (fake money)
- ✅ Auto-log in database
- ✅ Auto-update portfolio
- ✅ Return order details

---

## 🖥️ Frontend Options

### Option A: Use Python Requests (Quick Test - 5 min)

Create `test_trading.py`:
```python
import requests

API_BASE = "http://localhost:3000"
API_KEY = "your_api_key_here"

headers = {
    "X-API-Key": API_KEY,
    "Content-Type": "application/json"
}

# Get account balance
account = requests.get(f"{API_BASE}/api/broker/account", headers=headers).json()
print(f"Balance: ${account['data']['buying_power']}")

# Execute trade
trade = {
    "symbol": "AAPL",
    "action": "buy",
    "shares": 5,
    "notes": "Paper trading test"
}

result = requests.post(
    f"{API_BASE}/api/broker/execute",
    json=trade,
    headers=headers
).json()

print(f"Order ID: {result['data']['id']}")
print(f"Status: {result['data']['status']}")
```

Run it:
```bash
python test_trading.py
```

### Option B: Use Existing Portfolio Dashboard (Manual for now)

The current `portfolio_app.py` works, but you'll need to:
1. Use the API endpoints directly (curl/Postman)
2. Trades will auto-log
3. Portfolio updates automatically

**To add "Execute" buttons to the dashboard**, I can either:
- Create a new simpler trading-focused dashboard (2 hours)
- Enhance the existing portfolio_app.py (3 hours)

---

## 📋 Available API Endpoints

### Broker Endpoints (All Working!)

#### Get Account Info
```bash
GET /api/broker/account

Response:
{
  "success": true,
  "data": {
    "buying_power": "100000.00",
    "cash": "100000.00",
    "portfolio_value": "100000.00",
    "pattern_day_trader": false,
    "trading_blocked": false
  }
}
```

#### Execute Trade
```bash
POST /api/broker/execute

Body:
{
  "symbol": "AAPL",
  "action": "buy",  // or "sell"
  "shares": 10,
  "notes": "Optional notes"
}

Response:
{
  "success": true,
  "data": {
    "id": "order-uuid",
    "symbol": "AAPL",
    "status": "filled",
    "filled_avg_price": "178.50",
    "filled_quantity": "10"
  }
}
```

#### Get Broker Positions
```bash
GET /api/broker/positions

Response: List of positions from Alpaca
```

#### Get Orders
```bash
GET /api/broker/orders

Response: Last 50 orders from Alpaca
```

#### Cancel Order
```bash
POST /api/broker/orders/:id/cancel
```

#### Close Position
```bash
DELETE /api/broker/positions/AAPL

Body:
{
  "notes": "Taking profits"
}
```

---

## 🎨 Simple Trading Script

Here's a complete script to trade based on InvestIQ signals:

```python
#!/usr/bin/env python3
"""
Simple Paper Trading Script
Fetches InvestIQ signals and executes trades via Alpaca
"""

import requests
import time

API_BASE = "http://localhost:3000"
API_KEY = "your_api_key_here"

headers = {
    "X-API-Key": API_KEY,
    "Content-Type": "application/json"
}

def get_account():
    """Get account balance"""
    response = requests.get(f"{API_BASE}/api/broker/account", headers=headers)
    return response.json()['data']

def get_actions():
    """Get action inbox items"""
    response = requests.get(f"{API_BASE}/api/alerts/actions", headers=headers)
    return response.json()['data']

def execute_trade(symbol, action, shares):
    """Execute a trade"""
    trade = {
        "symbol": symbol,
        "action": action,
        "shares": shares,
        "notes": f"Auto-executed from signal"
    }

    response = requests.post(
        f"{API_BASE}/api/broker/execute",
        json=trade,
        headers=headers
    )
    return response.json()

def main():
    # Show account
    account = get_account()
    print(f"\n💰 Paper Trading Account")
    print(f"   Balance: ${float(account['buying_power']):,.2f}")
    print(f"   Portfolio Value: ${float(account['portfolio_value']):,.2f}")

    # Get signals
    actions = get_actions()
    print(f"\n🔔 {len(actions)} Action Items")

    for action in actions:
        if action['action_type'] == 'buy' and action['confidence'] > 0.80:
            symbol = action['symbol']
            print(f"\n🚀 Strong Buy Signal: {symbol} ({action['confidence']*100:.0f}% confidence)")

            # Ask user
            confirm = input(f"   Execute trade? Buy 10 shares of {symbol}? (y/n): ")

            if confirm.lower() == 'y':
                result = execute_trade(symbol, "buy", 10)
                if result['success']:
                    order = result['data']
                    print(f"   ✅ Order {order['id']}: {order['status']}")
                else:
                    print(f"   ❌ Error: {result.get('error')}")
            else:
                print("   ⏭️  Skipped")

if __name__ == "__main__":
    main()
```

Save as `trade.py` and run:
```bash
python trade.py
```

---

## 📊 Check Your Results

After executing trades:

**1. View in Alpaca Dashboard:**
- Go to https://app.alpaca.markets/paper/dashboard
- See your paper trades

**2. Check Database:**
```bash
sqlite3 portfolio.db "SELECT * FROM trades ORDER BY created_at DESC LIMIT 5;"
```

**3. Check Portfolio:**
```bash
curl -H "X-API-Key: your_key" http://localhost:3000/api/portfolio
```

---

## 🎯 What You Can Do Right Now

### Fully Working:
- ✅ Execute trades via API
- ✅ Auto-logging to database
- ✅ Auto-update portfolio
- ✅ Get account balance
- ✅ View positions
- ✅ View orders
- ✅ Cancel orders
- ✅ Close positions

### Needs Frontend (Optional):
- ❌ Click buttons in dashboard (use API for now)
- ❌ Visual confirmation dialogs (use scripts for now)

---

## 🚀 Next Steps

**You can start paper trading RIGHT NOW using:**

1. **Python scripts** (easiest - examples above)
2. **curl commands** (for testing)
3. **Postman/Insomnia** (for API exploration)

**Want me to build a frontend?**

I can create either:
- **Simple trading dashboard** (2 hours) - Just execute trades from signals
- **Full dashboard integration** (3 hours) - Add to existing portfolio app

**Or you can use the API directly - it's fully functional!**

---

## ✅ Your System Status

```
Backend:
✅ Alpaca integrated
✅ API endpoints working
✅ Auto-logging active
✅ Paper trading ready

Account:
✅ $100,000 paper money
✅ Commission-free
✅ Real market prices

What's Working:
✅ Execute trades
✅ Track portfolio
✅ Log history
✅ Get signals

What's Missing:
⚠️  Visual dashboard (optional - API works!)
```

---

## 🎉 You're Ready!

**Start paper trading now with the Python script above, or tell me if you want the visual dashboard built!**

Your choice:
1. Use Python scripts (works now, no waiting)
2. Wait 2-3 hours for me to build visual dashboard

**What do you prefer?** 🤔
