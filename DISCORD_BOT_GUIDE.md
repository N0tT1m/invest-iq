# InvestIQ Discord Bot - Complete Guide

## 🤖 Overview

The InvestIQ Discord Bot brings professional stock analysis directly into your Discord server with:
- ✅ **Buy/Sell Recommendations** - Clear signals from Strong Buy to Strong Sell
- ✅ **Visual Charts** - Professional candlestick charts with technical indicators
- ✅ **4 Analysis Types** - Technical, Fundamental, Quantitative, and Sentiment
- ✅ **Real-time Data** - Live data from Polygon.io

## 📋 Commands

### `!iq analyze <SYMBOL>`
**Get comprehensive analysis with chart**

**Example:**
```
!iq analyze AAPL
```

**Bot Response:**
1. **Chart Image** (PNG file):
   - Candlestick chart with 90 days of data
   - SMA 20 (blue line)
   - RSI indicator (70/30 overbought/oversold lines)
   - MACD with histogram
   - Color-coded by signal (Green=Buy, Red=Sell, Yellow=Neutral)

2. **Text Analysis**:
   ```
   🚀 Analysis for AAPL

   Overall Signal: Buy
   Recommendation: Buy (confidence: moderate - 75%)

   📊 Technical: Buy (80% confidence)
      + RSI Oversold, + MACD Bullish Cross, + Price Above MAs

   💼 Fundamental: WeakBuy (60% confidence)
      + Low P/E Ratio, + Strong ROE, + Positive Cash Flow

   🔢 Quantitative: Buy (70% confidence)
      Sharpe Ratio: 1.45

   📰 Sentiment: Neutral (65% confidence)
      Neutral news sentiment (12 positive, 8 negative, 5 neutral)

   Analysis performed at 2025-10-06 18:30 UTC
   ```

### `!iq chart <SYMBOL>`
**Get chart only (faster response)**

**Example:**
```
!iq chart TSLA
```

**Bot Response:**
- Chart image with caption showing signal and confidence
- No detailed text analysis
- Faster than `analyze` command

### `!iq help`
**Show help message**

Displays all available commands and features.

## 📊 Chart Features

### Main Price Chart (Top 60%)
- **Candlestick Chart**:
  - Green candles: Price increased
  - Red candles: Price decreased
  - Shows Open, High, Low, Close prices
- **SMA 20** (Blue line): 20-period simple moving average
- **Signal Color**: Chart title color matches the signal
  - Green: Buy signals
  - Red: Sell signals
  - Yellow: Neutral
  - Cyan: Weak Buy
  - Orange: Weak Sell

### RSI Chart (Middle 20%)
- **RSI Line** (Cyan): Relative Strength Index
- **Reference Lines**:
  - Red (70): Overbought zone
  - Green (30): Oversold zone
  - Gray (50): Neutral line
- **Interpretation**:
  - RSI > 70: Stock may be overbought
  - RSI < 30: Stock may be oversold

### MACD Chart (Bottom 20%)
- **MACD Line** (Blue): Fast EMA - Slow EMA
- **Signal Line** (Orange): 9-period EMA
- **Histogram** (Bars): MACD - Signal
  - Green bars: Bullish momentum
  - Red bars: Bearish momentum
- **Zero Line** (Gray): Momentum reference

## 🎯 Buy/Sell Signals

### Signal Types

| Signal | Emoji | Meaning | Action |
|--------|-------|---------|--------|
| **Strong Buy** | 🚀 | High conviction buy | Consider buying |
| **Buy** | 📈 | Clear buy signal | Consider buying |
| **Weak Buy** | ↗️ | Slight buy bias | Cautious buy |
| **Neutral** | ➡️ | No clear direction | Hold/Wait |
| **Weak Sell** | ↘️ | Slight sell bias | Watch closely |
| **Sell** | 📉 | Clear sell signal | Consider selling |
| **Strong Sell** | ⚠️ | High conviction sell | Consider selling |

### How Signals Are Determined

The bot combines 4 analysis engines with weighted scoring:

1. **Technical Analysis** (30% weight)
   - Indicators: RSI, MACD, Moving Averages, Bollinger Bands, Stochastic
   - Patterns: Candlestick patterns (Hammer, Engulfing, Doji, etc.)
   - Trends: Uptrend/Downtrend/Sideways

2. **Fundamental Analysis** (35% weight)
   - P/E Ratio, ROE, Profit Margins
   - Debt levels, Liquidity ratios
   - Cash flow analysis

3. **Quantitative Analysis** (25% weight)
   - Sharpe Ratio (risk-adjusted returns)
   - Volatility and Max Drawdown
   - Beta, VaR, Win Rate

4. **Sentiment Analysis** (10% weight)
   - News article sentiment
   - Positive/Negative/Neutral breakdown
   - Recency-weighted scoring

**Overall Signal** = Weighted average of all four analyses

## 🚀 Setup Guide

### Prerequisites
- Discord bot token
- Polygon.io API key
- API server running on localhost:3000

### Step 1: Create Discord Bot

1. Go to https://discord.com/developers/applications
2. Click "New Application" → Name it "InvestIQ"
3. Go to "Bot" tab → "Add Bot"
4. **Important**: Enable "Message Content Intent"
5. Copy the bot token

### Step 2: Invite to Server

1. OAuth2 → URL Generator
2. Select scopes: `bot`
3. Select permissions:
   - Send Messages
   - Attach Files
   - Read Message History
   - Use Slash Commands
4. Copy generated URL
5. Open in browser and select your server

### Step 3: Configure

```bash
# Edit .env file
DISCORD_BOT_TOKEN=your_discord_token_here
POLYGON_API_KEY=your_polygon_key_here
```

### Step 4: Start the Bot

```bash
# Make sure API server is running first
cargo run --release --bin api-server

# In another terminal
cargo run --release --bin discord-bot
```

You should see:
```
Discord bot starting...
📊 Chart generation enabled!
InvestIQ is connected and ready!
```

### Step 5: Test in Discord

```
!iq analyze AAPL
!iq chart MSFT
!iq help
```

## 💡 Usage Tips

### Best Practices

1. **Use `analyze` for decisions**
   - Full analysis with chart and all metrics
   - Best for investment research

2. **Use `chart` for quick checks**
   - Faster response
   - Good for monitoring multiple stocks

3. **Check multiple timeframes**
   - Bot uses 90 days of daily data by default
   - Good for swing trading and investing
   - For day trading, use the Dash dashboard

4. **Combine with your analysis**
   - Bot is a tool, not a crystal ball
   - Always do your own research
   - Consider your risk tolerance

### Common Workflows

**Research Mode**
```
!iq analyze AAPL
!iq analyze MSFT
!iq analyze GOOGL
[Compare signals and charts]
```

**Quick Check**
```
!iq chart TSLA
[Just want to see if trend changed]
```

**Sharing with Team**
```
Member 1: What do you think about NVDA?
Member 2: !iq analyze NVDA
[Everyone sees same analysis and chart]
```

## 📈 Reading the Charts

### Candlestick Patterns to Watch

**Bullish Patterns** (Look for on chart):
- **Hammer**: Long lower wick, small body at top
- **Bullish Engulfing**: Large green candle engulfs previous red
- **Morning Star**: Three-candle reversal pattern
- **Price touches SMA 20 from above**: Potential support

**Bearish Patterns**:
- **Shooting Star**: Long upper wick, small body at bottom
- **Bearish Engulfing**: Large red candle engulfs previous green
- **Evening Star**: Three-candle reversal pattern
- **Price breaks below SMA 20**: Potential weakness

### Indicator Signals

**RSI Signals**:
- **RSI < 30 + Bullish**: Strong buy signal (oversold)
- **RSI > 70 + Bearish**: Strong sell signal (overbought)
- **RSI divergence**: Price makes new high but RSI doesn't = bearish

**MACD Signals**:
- **MACD crosses above Signal**: Bullish (green histogram)
- **MACD crosses below Signal**: Bearish (red histogram)
- **Histogram expanding**: Momentum increasing
- **Histogram contracting**: Momentum decreasing

## ⚙️ Technical Details

### Chart Generation
- **Resolution**: 1200x900 pixels
- **Format**: PNG
- **Data**: 90 days of daily bars
- **Generation Time**: ~2-3 seconds
- **Storage**: Temporary file (auto-deleted after upload)

### Performance
- **Analyze command**: 3-5 seconds (chart + analysis)
- **Chart command**: 2-4 seconds (chart only)
- **Concurrent requests**: Handled via async Rust
- **Rate limiting**: Shares Polygon API limits with main system

### Error Handling
- If chart generation fails: Sends text analysis only
- If API is down: Error message with details
- If symbol not found: Clear error message
- Typing indicator during processing

## 🔧 Customization

### Change Chart Timeframe

Edit `crates/discord-bot/src/main.rs`:
```rust
// Line 199 - Change days parameter
let url = format!("{}/api/bars/{}?timeframe=1d&days=90", ...);
// Change to days=30 for 30 days, days=180 for 6 months, etc.
```

### Change Chart Size

Edit `crates/discord-bot/src/charts.rs`:
```rust
// Lines 4-5
const CHART_WIDTH: u32 = 1200;  // Change width
const CHART_HEIGHT: u32 = 900;  // Change height
```

### Add Custom Indicators

In `charts.rs`, add your indicator calculation and drawing:
```rust
// Add function to calculate indicator
fn calculate_my_indicator(bars: &[Bar]) -> Vec<f64> { ... }

// Add drawing in draw_price_chart or create new panel
```

## 🐛 Troubleshooting

### Bot not responding
- ✅ Check Message Content Intent is enabled
- ✅ Verify bot has permissions in channel
- ✅ Check bot is online in Discord
- ✅ Look at bot logs for errors

### Charts not generating
- ✅ Make sure API server is running (localhost:3000)
- ✅ Check `/tmp` directory is writable
- ✅ Verify plotters library is installed
- ✅ Check logs for specific errors

### "Insufficient data" errors
- ✅ Symbol may not have 90 days of data
- ✅ Try more liquid stocks (AAPL, MSFT, GOOGL)
- ✅ Check Polygon API has data for symbol

### Slow responses
- ✅ Normal: 3-5 seconds for full analysis
- ✅ Polygon free tier: 5 calls/minute limit
- ✅ Use `chart` command for faster responses
- ✅ Consider caching in API server

### Discord "Message too long" error
- ✅ Analysis text is usually under 2000 chars
- ✅ If happens, bot automatically truncates
- ✅ Chart sent as separate image attachment

## 📊 Example Sessions

### Investment Research
```
@Investor: !iq analyze AAPL
@InvestIQ: [Chart + Full Analysis: Buy (75% confidence)]

@Investor: !iq analyze MSFT
@InvestIQ: [Chart + Full Analysis: Strong Buy (88% confidence)]

@Investor: !iq analyze TSLA
@InvestIQ: [Chart + Full Analysis: Sell (72% confidence)]

Decision: Buy MSFT (highest conviction), skip TSLA
```

### Day Trading Alert
```
@Trader: !iq chart SPY
@InvestIQ: [Chart showing RSI oversold + MACD bullish cross]
@Trader: Good entry point! Going long.
```

### Team Discussion
```
@Member1: NVDA earnings tomorrow, thoughts?
@Member2: !iq analyze NVDA
@InvestIQ: [Shows neutral signal, high volatility metrics]
@Member3: Risky play before earnings, I'll wait
```

## 🎓 Learning Resources

### Understanding Signals
- **Strong signals** (>80% confidence): Multiple confirming indicators
- **Moderate signals** (60-80%): Some confirming indicators
- **Weak signals** (<60%): Mixed or limited data

### Chart Reading Practice
1. Look at overall trend (SMA direction)
2. Check RSI for extremes
3. Watch for MACD crossovers
4. Identify candlestick patterns
5. Consider volume (shown in main chart)

### Risk Management
- Never invest based on one signal
- Always use stop losses
- Consider position sizing
- Diversify your portfolio
- Understand your risk tolerance

## ⚠️ Important Disclaimers

### Not Financial Advice
- This bot is for educational purposes only
- Not a substitute for professional financial advice
- Past performance doesn't guarantee future results
- All investments carry risk of loss

### Data Limitations
- Free Polygon tier has 5-minute delays
- Historical data may have gaps
- News sentiment is keyword-based (not ML)
- Analysis based on historical patterns

### Use Responsibly
- Always do your own research
- Consult a financial advisor
- Understand what you're buying
- Don't invest money you can't afford to lose

## 🎉 Summary

The Discord bot provides:
- ✅ Clear Buy/Sell recommendations
- ✅ Professional chart images
- ✅ 4 types of analysis combined
- ✅ Easy sharing with teams
- ✅ Fast, convenient access

Perfect for:
- Trading communities
- Investment groups
- Personal research
- Quick market checks

**Ready to use!** Just type `!iq analyze AAPL` in Discord! 🚀

---

For more information, see:
- **README.md** - Project overview
- **SETUP_GUIDE.md** - Full setup instructions
- **ARCHITECTURE.md** - Technical details

Built with ❤️ in Rust
