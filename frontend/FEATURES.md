# InvestIQ Dashboard Features

## 🎨 Visual Guide

### Dashboard Overview

The InvestIQ Dashboard provides a comprehensive, interactive interface for stock analysis with real-time data visualization and multi-dimensional analysis.

## 📊 Main Components

### 1. Control Panel
**Location**: Top of page

**Features**:
- **Symbol Input**: Enter any stock ticker (AAPL, MSFT, TSLA, etc.)
- **Timeframe Selector**: Choose from multiple timeframes
  - `1m` - 1 Minute (intraday trading)
  - `5m` - 5 Minutes (scalping)
  - `15m` - 15 Minutes (day trading)
  - `1h` - 1 Hour (swing trading)
  - `1d` - 1 Day (position trading) - **Default**
  - `1w` - 1 Week (long-term)
- **Days Back**: Set lookback period (1-365 days)
- **Analyze Button**: Trigger new analysis
- **Refresh Button**: Update with latest data

### 2. Overall Signal Card
**Location**: Below controls

**Displays**:
- **Signal Type**: StrongBuy, Buy, WeakBuy, Neutral, WeakSell, Sell, StrongSell
- **Signal Emoji**: Visual indicator (🚀, 📈, ↗️, ➡️, ↘️, 📉, ⚠️)
- **Confidence Score**: 0-100% with color-coded progress bar
- **Recommendation**: Human-readable action with confidence level
- **Metadata**: Symbol, timestamp of analysis

**Color Coding**:
- Green (Success): Buy signals
- Yellow (Warning): Neutral or weak signals
- Red (Danger): Sell signals

### 3. Price Chart with Volume
**Location**: Main chart area

**Features**:
- **Candlestick Chart**:
  - Green candles: Price increased
  - Red candles: Price decreased
  - Hover for OHLC data
- **Bollinger Bands**:
  - Upper band (gray): Overbought threshold
  - Middle band (orange): 20-period SMA
  - Lower band (gray): Oversold threshold
- **Volume Bars**:
  - Green: Buying pressure
  - Red: Selling pressure
- **Interactive Tools**:
  - Zoom: Click and drag
  - Pan: Shift + drag
  - Reset: Double-click
  - Box select: Draw rectangle
  - Auto-scale: Click home icon

### 4. Technical Indicators

#### RSI (Relative Strength Index)
**Location**: Below main chart

**What it shows**:
- Momentum oscillator (0-100 scale)
- Overbought zone: Above 70 (red line)
- Oversold zone: Below 30 (green line)
- Neutral: Around 50 (gray line)

**How to read**:
- RSI > 70: Stock may be overbought (potential sell)
- RSI < 30: Stock may be oversold (potential buy)
- Divergences: Price vs RSI direction can signal reversals

**Current Value**: Annotated on chart with cyan marker

#### MACD (Moving Average Convergence Divergence)
**Location**: Below RSI chart

**Components**:
- **MACD Line** (blue): Fast EMA - Slow EMA
- **Signal Line** (orange): 9-period EMA of MACD
- **Histogram** (bars): MACD - Signal
  - Green bars: Bullish momentum
  - Red bars: Bearish momentum

**How to read**:
- MACD crosses above Signal: Bullish (potential buy)
- MACD crosses below Signal: Bearish (potential sell)
- Histogram expanding: Momentum increasing
- Histogram contracting: Momentum decreasing

### 5. Analysis Cards

#### Technical Analysis Card
**Location**: Left side, middle row

**Contains**:
- **Signal**: Buy/Sell recommendation
- **Confidence**: Percentage based on signal strength
- **Reasoning**: List of all technical signals detected
  - Example: "+ RSI Oversold, + MACD Bullish Cross, - Price Below MAs"
- **Key Metrics**:
  - Current RSI value
  - MACD histogram value
  - Detected trend (Uptrend/Downtrend/Sideways)
  - Number of candlestick patterns found
  - Total signal count

**Interpretation**:
- High confidence (>70%): Multiple confirming signals
- Low confidence (<40%): Mixed or weak signals
- More signals = more data points analyzed

#### Fundamental Analysis Card
**Location**: Right side, middle row

**Contains**:
- **Signal**: Based on company financials
- **Confidence**: Based on data availability
- **Reasoning**: Financial health indicators
- **Financial Metrics**:
  - **P/E Ratio**: Price-to-Earnings
    - < 15: Potentially undervalued
    - 15-25: Fair value
    - > 25: Potentially overvalued
  - **ROE**: Return on Equity
    - > 15%: Strong
    - < 5%: Weak
  - **Profit Margin**: Net income / Revenue
    - > 20%: High margin
    - < 5%: Low margin
  - **Debt/Equity**: Leverage ratio
    - < 0.5: Conservative
    - > 2.0: Aggressive
  - **Current Ratio**: Liquidity
    - > 1.5: Healthy
    - < 1.0: Concerning

**Best For**:
- Long-term investing
- Value investing
- Comparing companies in same sector

#### Quantitative Analysis Card
**Location**: Left side, bottom row

**Contains**:
- **Signal**: Based on statistical metrics
- **Confidence**: Set at 70% (moderate)
- **Reasoning**: Risk and return characteristics
- **Risk Metrics**:
  - **Sharpe Ratio**: Risk-adjusted returns
    - > 1.0: Good risk/reward
    - < 0.0: Poor risk/reward
  - **Volatility**: Annual price fluctuation
    - < 20%: Low volatility
    - > 40%: High volatility
  - **Max Drawdown**: Worst peak-to-trough decline
    - < 10%: Low drawdown
    - > 25%: High drawdown
  - **Beta**: Market correlation
    - > 1.2: More volatile than market
    - < 0.8: Less volatile than market
  - **VaR (95%)**: Value at Risk
    - Maximum expected loss 95% of time
  - **Win Rate**: Historical momentum accuracy

**Best For**:
- Risk assessment
- Portfolio optimization
- Comparing volatility
- Understanding downside risk

#### Sentiment Analysis Card
**Location**: Right side, bottom row

**Contains**:
- **Signal**: Based on news sentiment
- **Confidence**: Based on article count and consistency
- **Reasoning**: News sentiment breakdown
- **Metrics**:
  - **Positive Articles**: Bullish news count
  - **Neutral Articles**: Neutral news count
  - **Negative Articles**: Bearish news count
  - **Total Articles**: Sample size
  - **Sentiment Score**: -100 to +100 scale

**How it works**:
- Analyzes recent news articles (up to 50)
- Scans for positive/negative keywords
- Weights recent news more heavily
- Scores title higher than description

**Best For**:
- Short-term trading
- Event-driven strategies
- Detecting market sentiment shifts
- Validating other signals

## 🎯 Use Cases

### Day Trading
**Setup**:
- Timeframe: 1m, 5m, or 15m
- Days back: 1-5 days
- Focus: Technical + Sentiment

**Look for**:
- RSI extremes (>70 or <30)
- MACD crossovers
- Volume spikes
- Recent news events

### Swing Trading
**Setup**:
- Timeframe: 1h or 1d
- Days back: 30-90 days
- Focus: Technical + Quantitative

**Look for**:
- Trend direction
- Support/resistance levels
- Bollinger Band touches
- Momentum shifts

### Long-term Investing
**Setup**:
- Timeframe: 1d or 1w
- Days back: 90-365 days
- Focus: Fundamental + Quantitative

**Look for**:
- Strong fundamentals (ROE, margins)
- Low volatility
- Good risk-adjusted returns
- Positive long-term trend

## 💡 Pro Tips

### 1. Combine Multiple Signals
Don't rely on just one analysis type. Best results come from:
- Technical confirms the entry point
- Fundamental validates the company
- Quantitative assesses the risk
- Sentiment provides context

### 2. Use Appropriate Timeframes
- Intraday (1m-15m): Lots of noise, use for scalping only
- Short-term (1h-1d): Good for swing trading
- Long-term (1d-1w): Best for position trading

### 3. Watch for Confirmation
Strong signals occur when multiple indicators agree:
- RSI oversold + MACD bullish cross + positive news = Strong buy
- Price above Bollinger upper + RSI overbought + high volatility = Strong sell

### 4. Monitor Confidence Scores
- Overall confidence > 70%: High conviction
- Overall confidence 40-70%: Moderate conviction
- Overall confidence < 40%: Low conviction, be cautious

### 5. Use the Refresh Button
Market conditions change:
- Refresh during market hours for latest data
- Cache is 5 minutes by default
- Force refresh gets newest analysis

## ⚡ Keyboard Shortcuts

While interacting with charts:
- **Double-click**: Reset zoom
- **Shift + drag**: Pan
- **Click + drag**: Zoom to selection
- **Hover**: Show data tooltip

## 🔄 Auto-Update Workflow

Recommended workflow for active trading:

1. **Initial Analysis**
   - Enter symbol
   - Set timeframe
   - Click Analyze

2. **Review Results**
   - Check overall signal
   - Examine all 4 analysis types
   - Study the charts

3. **Monitor Changes**
   - Click Refresh every 5-10 minutes
   - Watch for signal changes
   - Track price movement

4. **Make Decisions**
   - Combine with your strategy
   - Set entry/exit points
   - Manage risk appropriately

## 📈 Chart Patterns to Watch

### Bullish Patterns (Look for on main chart)
- **Hammer**: Long lower wick, small body
- **Engulfing**: Current candle engulfs previous
- **Morning Star**: Three-candle reversal
- **Price touches lower Bollinger Band**

### Bearish Patterns
- **Shooting Star**: Long upper wick, small body
- **Dark Cloud Cover**: Bearish engulfing
- **Evening Star**: Three-candle reversal
- **Price touches upper Bollinger Band**

## 🎓 Learning Resources

### Understanding Technical Indicators
- **RSI**: Measures momentum, 14-period default
- **MACD**: Measures trend and momentum
- **Bollinger Bands**: Measures volatility
- **SMA**: Simple moving average, trend indicator

### Understanding Fundamental Metrics
- **P/E Ratio**: How much you pay per $1 of earnings
- **ROE**: How efficiently company uses equity
- **Profit Margin**: How much profit per sale
- **Current Ratio**: Can company pay short-term debts?

### Understanding Risk Metrics
- **Sharpe Ratio**: Return per unit of risk
- **Volatility**: How much price fluctuates
- **Beta**: Correlation with market
- **Max Drawdown**: Worst historical loss

## 🚨 Warnings & Disclaimers

### ⚠️ Important Notes

1. **Not Financial Advice**: This tool is for educational purposes only
2. **Past Performance**: Does not guarantee future results
3. **Market Risk**: All investments carry risk of loss
4. **Data Accuracy**: Analysis depends on data quality
5. **Latency**: Free tier has 5-minute delays
6. **Consult Professional**: Always consult a financial advisor

### Common Pitfalls to Avoid

1. **Over-trading**: Don't trade on every signal
2. **Ignoring Risk**: Always consider max drawdown and volatility
3. **Chasing Performance**: Don't buy just because it went up
4. **Ignoring Fundamentals**: Technical analysis alone is risky
5. **FOMO**: Fear of missing out leads to bad decisions

## 🔮 Future Features (Roadmap)

- [ ] Multiple symbol comparison
- [ ] Custom indicator builder
- [ ] Backtesting simulator
- [ ] Price alerts
- [ ] Portfolio tracking
- [ ] Export to CSV/Excel
- [ ] Mobile responsive design
- [ ] Dark/Light theme toggle
- [ ] Save favorite symbols
- [ ] Historical analysis archive

---

**Remember**: Use this tool as one input in your decision-making process. Always do your own research and understand the risks involved in trading.

Happy analyzing! 📊
