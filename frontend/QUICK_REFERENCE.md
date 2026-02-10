# InvestIQ Quick Reference Card

## 🚀 Getting Started

### Start the Dashboards

```bash
# Analysis Dashboard
cd frontend
python app_enhanced.py
# → http://localhost:8050

# Trading Dashboard
export API_KEY=your_key_here
python trading_dashboard_enhanced.py
# → http://localhost:8052
```

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| <kbd>/</kbd> | Focus search box |
| <kbd>Enter</kbd> | Analyze stock |
| <kbd>R</kbd> | Refresh data |
| <kbd>W</kbd> | Toggle watchlist |
| <kbd>H</kbd> | Show help |
| <kbd>E</kbd> | Export data |
| <kbd>Esc</kbd> | Close modals |

## 📊 Understanding Metrics

### Technical Indicators

**RSI (Relative Strength Index)**
- Range: 0-100
- < 30 = Oversold (potential buy)
- \> 70 = Overbought (potential sell)
- 50 = Neutral

**MACD (Moving Average Convergence Divergence)**
- Bullish: MACD crosses above signal line ↑
- Bearish: MACD crosses below signal line ↓
- Histogram shows momentum strength

**Bollinger Bands**
- Upper band = Resistance
- Lower band = Support
- Price touching upper = Overbought
- Price touching lower = Oversold

### Fundamental Metrics

**P/E Ratio (Price-to-Earnings)**
- Lower = Potentially undervalued
- Compare to industry average
- < 15 = Value stock
- \> 25 = Growth stock

**ROE (Return on Equity)**
- Measures profitability
- Higher = Better
- \> 15% = Good
- \> 20% = Excellent

**Debt/Equity Ratio**
- Measures financial leverage
- Lower = Less risky
- < 1.0 = Conservative
- \> 2.0 = Aggressive

### Risk Metrics

**Sharpe Ratio**
- Risk-adjusted return
- \> 1.0 = Good
- \> 2.0 = Very good
- \> 3.0 = Excellent

**Volatility**
- Price fluctuation measure
- Lower = More stable
- Higher = More risky
- Annualized percentage

**Max Drawdown**
- Worst historical decline
- Percentage from peak to trough
- Lower = Better risk management

## 🎯 Common Workflows

### Analyze a Stock

1. Press <kbd>/</kbd> to focus search
2. Type symbol (e.g., AAPL)
3. Select timeframe (1D, 1W, 1M, etc.)
4. Click **🔍 Analyze** or press <kbd>Enter</kbd>
5. Review tabs:
   - 📊 Charts & Analysis
   - 📈 Technical Deep Dive
   - 💼 Fundamental Analysis
   - 🔢 Risk & Quant
   - 📰 News & Sentiment

### Build a Watchlist

1. Analyze a stock
2. Click ⭐ star button
3. Access watchlist: Click "Watchlist" in navbar or press <kbd>W</kbd>
4. Click "View" to analyze from watchlist
5. Remove: Click "Remove" button

### Compare Stocks

1. Analyze first stock (e.g., AAPL)
2. Click **+ Add to Compare**
3. Search and analyze second stock (e.g., MSFT)
4. Click **+ Add to Compare**
5. Go to **🔍 Compare Stocks** tab
6. View normalized comparison chart
7. Clear: Click "Clear All" in banner

### Execute a Trade

1. Go to Trading Dashboard (port 8052)
2. Review **Action Inbox**
3. Choose an action card
4. Click **Execute BUY/SELL**
5. Modal opens:
   - Review details
   - Enter number of shares
   - Choose order type
   - Review order summary
6. Click **Confirm Trade**
7. Success notification appears

### Export Analysis

1. Analyze a stock
2. Click 📥 export button
3. Choose format:
   - CSV (spreadsheet)
   - JSON (API format)
4. File downloads automatically

## 🎨 Customization

### Settings

Click ⚙️ in navbar to access:

**Display:**
- [ ] Dark Mode (default on)
- [ ] Show Advanced Metrics
- [ ] Compact View

**Refresh:**
- Auto-refresh interval: 10-300 seconds
- Manual refresh: Click 🔄 or press <kbd>R</kbd>

**Charts:**
- [ ] Show Volume (default on)
- [ ] Show Bollinger Bands (default on)
- [ ] Show Moving Averages (default on)

### Period Selection

Quick buttons for time periods:
- **1W** = 7 days
- **1M** = 30 days
- **3M** = 90 days (default)
- **6M** = 180 days
- **1Y** = 365 days

## 🔍 Signal Interpretation

### Overall Signals

| Signal | Meaning | Action |
|--------|---------|--------|
| 🚀 StrongBuy | Very bullish | Consider buying |
| 📈 Buy | Bullish | Consider buying |
| ↗️ WeakBuy | Slightly bullish | Watch closely |
| ➡️ Neutral | No clear direction | Hold/wait |
| ↘️ WeakSell | Slightly bearish | Watch closely |
| 📉 Sell | Bearish | Consider selling |
| ⚠️ StrongSell | Very bearish | Consider selling |

### Confidence Levels

- **80-100%** = High confidence ✅
- **60-79%** = Moderate confidence ⚠️
- **40-59%** = Low confidence 🤔
- **<40%** = Very low confidence ❌

## 📱 Mobile Tips

### Navigation
- Bottom nav bar for quick access
- Swipe cards left/right
- Pull down to refresh
- Tap and hold for options

### Touch Gestures
- **Tap** = Select
- **Double tap** = Zoom chart
- **Pinch** = Zoom in/out
- **Swipe** = Navigate cards

## ❗ Troubleshooting

### Can't Connect to API

**Error:** Cannot connect to server

**Fix:**
```bash
# Check API server is running
curl http://localhost:3000/health

# Start API server
cargo run --release --bin api-server
```

### Authentication Failed

**Error:** Invalid API key

**Fix:**
```bash
# Generate new key
openssl rand -hex 32

# Set environment variable
export API_KEY=your_new_key_here
```

### Charts Not Loading

**Error:** Charts appear blank

**Fix:**
1. Hard refresh: <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>R</kbd>
2. Clear browser cache
3. Check browser console for errors (F12)
4. Try different symbol

### Slow Performance

**Issue:** Dashboard is slow

**Fix:**
1. Reduce auto-refresh interval
2. Disable unused chart indicators
3. Close other browser tabs
4. Use compact view
5. Clear recent symbols history

## 💡 Pro Tips

### Speed Tips
- Use <kbd>/</kbd> to quickly search
- Enable auto-refresh for monitoring
- Add frequently traded stocks to watchlist
- Use keyboard shortcuts instead of mouse

### Analysis Tips
- Check multiple timeframes (1D, 1W, 1M)
- Compare similar stocks
- Look at all 4 analysis types
- Check historical performance
- Read news sentiment

### Trading Tips
- Review action inbox daily
- Check confidence levels
- Verify current prices
- Use limit orders for control
- Track portfolio daily
- Review trade history weekly

### Mobile Tips
- Enable desktop site for full features
- Use landscape mode for charts
- Save to home screen for quick access
- Enable notifications (future feature)

## 🆘 Getting Help

### Built-in Help
- Press <kbd>H</kbd> for help modal
- Hover over ? icons for tooltips
- Check welcome tour (first time)
- Read glossary in help modal

### Documentation
- [UX Improvements](./UX_IMPROVEMENTS.md)
- [Migration Guide](./MIGRATION_GUIDE.md)
- [Main README](../README.md)

### Support Channels
- **GitHub Issues**: Bug reports
- **Discussions**: Feature requests
- **Discord**: Community help
- **Email**: support@investiq.com

## 📌 Quick Links

| Resource | Link |
|----------|------|
| Analysis Dashboard | http://localhost:8050 |
| Trading Dashboard | http://localhost:8052 |
| API Health Check | http://localhost:3000/health |
| GitHub Repo | [Link] |
| Documentation | [Link] |

## ⚠️ Important Notes

### Disclaimers
- **Not financial advice**
- **Educational purposes only**
- **Paper trading = No real money**
- **Always do your research**
- **Consult a professional**

### Best Practices
- ✅ Diversify portfolio
- ✅ Set stop losses
- ✅ Review regularly
- ✅ Stay informed
- ✅ Manage risk
- ❌ Don't invest what you can't lose
- ❌ Don't trade emotionally
- ❌ Don't chase losses

## 🎓 Learning Resources

### Understanding Analysis
1. **Technical Analysis**
   - [Investopedia - Technical Analysis](https://www.investopedia.com/terms/t/technicalanalysis.asp)
   - Focus on charts, patterns, indicators

2. **Fundamental Analysis**
   - [Investopedia - Fundamental Analysis](https://www.investopedia.com/terms/f/fundamentalanalysis.asp)
   - Focus on financials, ratios, company health

3. **Risk Management**
   - [Risk Management Basics](https://www.investopedia.com/terms/r/riskmanagement.asp)
   - Position sizing, stop losses, diversification

### Practice Resources
- Paper trading account (InvestIQ)
- Stock market simulators
- Historical data analysis
- Backtesting strategies

## 📝 Cheat Sheet

```
Keyboard Shortcuts:
/ = Search | Enter = Analyze | R = Refresh
W = Watchlist | H = Help | E = Export | Esc = Close

Technical Signals:
RSI <30 = Buy | RSI >70 = Sell
MACD ↑ = Bull | MACD ↓ = Bear

Confidence:
>80% = High | 60-79% = Med | <60% = Low

Quick Actions:
Star = Watchlist | Plus = Compare | Download = Export
Refresh = Update | Settings = Customize

Remember:
✓ Not financial advice
✓ Educational only
✓ DYOR (Do Your Own Research)
```

---

**Print this page and keep it handy! 📄**

For the full guide, see [UX_IMPROVEMENTS.md](./UX_IMPROVEMENTS.md)
