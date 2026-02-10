#!/usr/bin/env python3
"""
InvestIQ Click-to-Trade Script
Simple command-line interface for paper trading
"""

import requests
import os
import sys
from datetime import datetime

# Configuration
API_BASE = os.getenv("API_BASE", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "")

if not API_KEY:
    print("ERROR: API_KEY environment variable not set!")
    print("Set it with: export API_KEY=your_key_here")
    sys.exit(1)

headers = {"X-API-Key": API_KEY, "Content-Type": "application/json"}

# Helper functions
def fetch_account():
    """Get account balance"""
    try:
        response = requests.get(f"{API_BASE}/api/broker/account", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', {})
    except Exception as e:
        print(f"❌ Error fetching account: {e}")
    return None

def fetch_actions():
    """Get action inbox items"""
    try:
        response = requests.get(f"{API_BASE}/api/alerts/actions", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', [])
    except Exception as e:
        print(f"❌ Error fetching actions: {e}")
    return []

def fetch_portfolio():
    """Get portfolio summary"""
    try:
        response = requests.get(f"{API_BASE}/api/portfolio", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', {})
    except Exception as e:
        print(f"❌ Error fetching portfolio: {e}")
    return None

def execute_trade(symbol, action, shares):
    """Execute a trade"""
    try:
        trade_data = {
            "symbol": symbol,
            "action": action,
            "shares": shares,
            "notes": f"Executed from click-to-trade script"
        }
        response = requests.post(
            f"{API_BASE}/api/broker/execute",
            json=trade_data,
            headers=headers,
            timeout=30
        )
        return response.json()
    except Exception as e:
        return {"success": False, "error": str(e)}

def show_banner():
    """Display welcome banner"""
    print("\n" + "="*60)
    print("💰 InvestIQ Click-to-Trade")
    print("="*60)

def show_account():
    """Display account information"""
    print("\n📊 Account Balance:")
    print("-" * 60)

    account = fetch_account()
    if not account:
        print("❌ Unable to fetch account information")
        return False

    buying_power = float(account.get('buying_power', 0))
    portfolio_value = float(account.get('portfolio_value', 0))
    cash = float(account.get('cash', 0))

    print(f"   💵 Cash: ${cash:,.2f}")
    print(f"   💰 Buying Power: ${buying_power:,.2f}")
    print(f"   📈 Portfolio Value: ${portfolio_value:,.2f}")
    print("-" * 60)

    return True

def show_portfolio():
    """Display current portfolio"""
    portfolio = fetch_portfolio()
    if not portfolio:
        return

    positions = portfolio.get('positions', [])
    if not positions:
        print("\nℹ️  No positions in portfolio")
        return

    print(f"\n📊 Portfolio ({len(positions)} positions):")
    print("-" * 60)

    for pos_data in positions:
        pos = pos_data.get('position', {})
        symbol = pos.get('symbol')
        shares = pos.get('shares', 0)
        entry_price = pos.get('entry_price', 0)
        current_price = pos_data.get('current_price', 0)
        unrealized_pnl = pos_data.get('unrealized_pnl', 0)
        unrealized_pnl_pct = pos_data.get('unrealized_pnl_percent', 0)

        pnl_icon = "📈" if unrealized_pnl >= 0 else "📉"

        print(f"\n   {symbol}:")
        print(f"      Shares: {shares:.2f}")
        print(f"      Entry: ${entry_price:.2f} → Now: ${current_price:.2f}")
        print(f"      {pnl_icon} P&L: ${unrealized_pnl:,.2f} ({unrealized_pnl_pct:+.2f}%)")

    total_value = portfolio.get('total_value', 0)
    total_pnl = portfolio.get('total_pnl', 0)
    total_pnl_pct = portfolio.get('total_pnl_percent', 0)

    print("\n   " + "-" * 50)
    print(f"   Total Value: ${total_value:,.2f}")
    print(f"   Total P&L: ${total_pnl:,.2f} ({total_pnl_pct:+.2f}%)")
    print("-" * 60)

def process_actions():
    """Display and process action items"""
    actions = fetch_actions()

    if not actions:
        print("\nℹ️  No action items at this time")
        return

    print(f"\n🔔 Action Inbox ({len(actions)} items):")
    print("=" * 60)

    trades_executed = 0

    for idx, action in enumerate(actions, 1):
        symbol = action.get('symbol')
        action_type = action.get('action_type', '').upper()
        signal = action.get('signal', '')
        confidence = action.get('confidence', 0) * 100
        current_price = action.get('current_price')
        target_price = action.get('target_price')
        description = action.get('description', '')
        in_portfolio = action.get('in_portfolio', False)

        # Display action
        print(f"\n{idx}. {symbol} - {signal}")
        print(f"   Action: {action_type}")
        print(f"   Confidence: {confidence:.0f}%")
        if current_price:
            print(f"   Current Price: ${current_price:.2f}")
        if target_price:
            print(f"   Target Price: ${target_price:.2f}")
        if in_portfolio:
            print(f"   ✓ Already in portfolio")
        print(f"   {description}")

        # Ask to execute
        print("\n   " + "-" * 50)
        execute = input(f"   Execute {action_type} for {symbol}? (y/n or number of shares): ").strip().lower()

        if execute == 'y':
            shares = 10  # Default
        elif execute.isdigit():
            shares = float(execute)
        else:
            print("   ⏭️  Skipped")
            continue

        # Confirm execution
        print(f"\n   Executing {action_type} {shares} shares of {symbol}...")

        result = execute_trade(symbol, action_type.lower(), shares)

        if result.get('success'):
            order = result.get('data', {})
            order_id = order.get('id', 'unknown')
            status = order.get('status', 'unknown')
            filled_price = order.get('filled_avg_price')

            print(f"   ✅ Trade executed successfully!")
            print(f"      Order ID: {order_id[:12]}...")
            print(f"      Status: {status}")
            if filled_price:
                print(f"      Fill Price: ${filled_price}")

            trades_executed += 1
        else:
            error = result.get('error', 'Unknown error')
            print(f"   ❌ Trade failed: {error}")

        print()

    return trades_executed

def main():
    """Main entry point"""
    show_banner()

    # Check connection
    if not show_account():
        print("\n❌ Unable to connect to API server")
        print("Make sure the API server is running: cargo run --release --bin api-server")
        sys.exit(1)

    # Show portfolio
    show_portfolio()

    # Process action items
    print("\n")
    trades_executed = process_actions()

    # Summary
    print("\n" + "=" * 60)
    if trades_executed > 0:
        print(f"✅ Session complete: {trades_executed} trade(s) executed")
        print("\nℹ️  Trades have been auto-logged and portfolio updated")
        print("   Check Alpaca dashboard: https://app.alpaca.markets/paper/dashboard")
    else:
        print("Session complete: No trades executed")
    print("=" * 60 + "\n")

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n⚠️  Interrupted by user")
        sys.exit(0)
    except Exception as e:
        print(f"\n\n❌ Unexpected error: {e}")
        sys.exit(1)
