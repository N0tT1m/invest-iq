# InvestIQ Architecture

## System Overview

InvestIQ is a high-performance stock analysis platform built in Rust using a modular, microservice-inspired architecture. The system combines multiple analysis engines to provide comprehensive trading insights.

```
┌─────────────────────────────────────────────────────────────┐
│                        Clients                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │   Dash   │  │  React   │  │ Discord  │  │   CLI    │   │
│  │ Frontend │  │ Frontend │  │   Bot    │  │  Tools   │   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘   │
└───────┼────────────┼─────────────┼─────────────┼──────────┘
        │            │             │             │
        └────────────┴─────────────┴─────────────┘
                     │
        ┌────────────▼────────────┐
        │   REST API (Axum)       │
        │   Port: 3000            │
        │   - CORS enabled        │
        │   - JSON responses      │
        │   - Redis/Memory cache  │
        └────────────┬────────────┘
                     │
        ┌────────────▼────────────┐
        │ Analysis Orchestrator   │
        │ - Coordinates engines   │
        │ - Combines results      │
        │ - Weighted scoring      │
        └────────────┬────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
┌───────▼───────┐         ┌───────▼───────┐
│  Data Layer   │         │ Analysis Layer│
│               │         │               │
│ ┌───────────┐ │         │ ┌───────────┐ │
│ │ Polygon   │ │         │ │Technical  │ │
│ │  Client   │ │         │ │ Analysis  │ │
│ └───────────┘ │         │ └───────────┘ │
│               │         │               │
│ - Bars/OHLCV  │         │ - Indicators  │
│ - Financials  │         │ - Patterns    │
│ - News        │         │ - Trends      │
│ - Ticker Info │         │               │
└───────────────┘         │ ┌───────────┐ │
                          │ │Fundamental│ │
┌───────────────┐         │ │ Analysis  │ │
│ Cache Layer   │         │ └───────────┘ │
│               │         │               │
│ ┌───────────┐ │         │ - P/E, ROE   │
│ │   Redis   │ │         │ - Debt ratios│
│ │(Optional) │ │         │ - Margins    │
│ └───────────┘ │         │               │
│               │         │ ┌───────────┐ │
│ ┌───────────┐ │         │ │Quantitative│ │
│ │  DashMap  │ │         │ │ Analysis  │ │
│ │(Fallback) │ │         │ └───────────┘ │
│ └───────────┘ │         │               │
└───────────────┘         │ - Sharpe     │
                          │ - Volatility │
                          │ - Drawdown   │
                          │               │
                          │ ┌───────────┐ │
                          │ │Sentiment  │ │
                          │ │ Analysis  │ │
                          │ └───────────┘ │
                          │               │
                          │ - News NLP   │
                          │ - Scoring    │
                          └───────────────┘
```

## Component Details

### 1. Analysis Core (`analysis-core`)

**Purpose**: Shared types, traits, and error handling

**Key Components**:
- `Bar`, `Quote`, `Trade` - Market data types
- `Financials` - Company financial data
- `AnalysisResult` - Standardized analysis output
- `SignalStrength` - Enum for buy/sell signals
- Analyzer traits for each engine type

**Dependencies**: None (foundation crate)

### 2. Polygon Client (`polygon-client`)

**Purpose**: HTTP client for Polygon.io API

**Features**:
- Async/await with `reqwest`
- Aggregates (OHLCV bars)
- Financials data
- News articles
- Ticker details

**API Calls**:
- `GET /v2/aggs/ticker/{symbol}/range/{multiplier}/{timespan}/{from}/{to}`
- `GET /vX/reference/financials`
- `GET /v2/reference/news`
- `GET /v3/reference/tickers/{symbol}`

### 3. Technical Analysis (`technical-analysis`)

**Purpose**: Chart-based analysis using price action and indicators

**Indicators Implemented**:
- SMA (Simple Moving Average)
- EMA (Exponential Moving Average)
- RSI (Relative Strength Index)
- MACD (Moving Average Convergence Divergence)
- Bollinger Bands
- Stochastic Oscillator
- ATR (Average True Range)
- OBV (On-Balance Volume)
- VWAP (Volume-Weighted Average Price)

**Pattern Recognition**:
- Candlestick patterns: Doji, Hammer, Shooting Star, Engulfing, etc.
- Trend detection using linear regression

**Signal Generation**:
- Weighted scoring based on multiple indicators
- Confidence calculated from number of signals
- Detailed reasoning for each signal

### 4. Fundamental Analysis (`fundamental-analysis`)

**Purpose**: Company financial health analysis

**Metrics Calculated**:
- P/E Ratio (Price-to-Earnings)
- ROE (Return on Equity)
- Profit Margin
- Debt-to-Equity Ratio
- Current Ratio (Liquidity)
- Operating Cash Flow

**Scoring**:
- Industry-standard thresholds
- Weighted by metric importance
- Conservative, moderate, and aggressive ranges

### 5. Quantitative Analysis (`quant-analysis`)

**Purpose**: Statistical and risk analysis

**Metrics**:
- **Sharpe Ratio**: Risk-adjusted returns
- **Volatility**: Annualized standard deviation
- **Maximum Drawdown**: Worst peak-to-trough decline
- **Beta**: Market correlation
- **VaR**: Value at Risk (95% confidence)
- **Win Rate**: Momentum strategy backtesting

**Statistics Library**: Uses `statrs` crate for calculations

### 6. Sentiment Analysis (`sentiment-analysis`)

**Purpose**: News and market sentiment analysis

**Approach**:
- Keyword-based sentiment scoring
- Positive/negative word dictionaries
- Recency weighting (recent news weighted higher)
- Title weighted more than description

**Limitations**:
- Currently uses simple keyword matching
- Future: Integrate with ML-based NLP models

### 7. Analysis Orchestrator (`analysis-orchestrator`)

**Purpose**: Coordinates all analysis engines and combines results

**Flow**:
1. Fetch data in parallel (bars, financials, news)
2. Run all applicable analyzers concurrently
3. Combine results with weighted scoring:
   - Technical: 30%
   - Fundamental: 35%
   - Quantitative: 25%
   - Sentiment: 10%
4. Generate unified recommendation

**Output**: `UnifiedAnalysis` with overall signal and confidence

### 8. API Server (`api-server`)

**Purpose**: REST API for frontend clients

**Technology**:
- Framework: Axum (Tokio-based)
- Serialization: Serde JSON
- CORS: Enabled for all origins
- Logging: Tracing + tracing-subscriber

**Caching**:
- Primary: Redis (distributed)
- Fallback: DashMap (in-memory)
- Configurable TTL per request

**Endpoints**:
```
GET  /health              - Health check
GET  /api/analyze/:symbol - Full analysis
GET  /api/bars/:symbol    - Historical bars
GET  /api/ticker/:symbol  - Ticker details
```

### 9. Discord Bot (`discord-bot`)

**Purpose**: Discord integration for chat-based analysis

**Technology**:
- Framework: Serenity
- Event-driven architecture
- Message content intents

**Commands**:
- `!iq analyze <SYMBOL>` - Get stock analysis
- `!iq help` - Show help message

**Features**:
- Formatted analysis with emojis
- Typing indicators during analysis
- Error handling and feedback

## Data Flow

### Analysis Request Flow

```
1. Client Request
   ↓
2. API Server (analyze_symbol)
   ↓
3. Check Cache (Redis/Memory)
   ├─ HIT → Return cached result
   └─ MISS → Continue
   ↓
4. Orchestrator.analyze()
   ↓
5. Parallel Data Fetch
   ├─ Polygon: Get bars (90 days)
   ├─ Polygon: Get financials
   └─ Polygon: Get news (50 articles)
   ↓
6. Parallel Analysis
   ├─ Technical Analysis (if enough bars)
   ├─ Fundamental Analysis (if financials available)
   ├─ Quant Analysis (if enough bars)
   └─ Sentiment Analysis (if news available)
   ↓
7. Combine Results
   - Calculate weighted score
   - Determine overall signal
   - Generate recommendation
   ↓
8. Cache Result (with TTL)
   ↓
9. Return to Client
```

## Performance Characteristics

### Async I/O
- All network operations are async
- Concurrent API calls to Polygon
- Parallel analysis execution

### Caching Strategy
- **Cache Key**: `analysis:{SYMBOL}`
- **Default TTL**: 300 seconds (5 minutes)
- **Backend**: Redis or in-memory
- **Eviction**: TTL-based

### Analysis Time
- **Cold cache**: 2-5 seconds (Polygon API calls)
- **Warm cache**: < 50ms (cached result)
- **Parallel execution**: Reduces latency by ~60%

### Memory Usage
- **API Server**: ~50MB baseline
- **Per analysis**: ~1-5MB (bars data)
- **Cache**: Depends on number of symbols tracked

## Scalability

### Horizontal Scaling
- **Stateless**: Can run multiple API instances
- **Shared Cache**: Redis enables distributed caching
- **Load Balancer**: Put instances behind nginx/HAProxy

### Vertical Scaling
- CPU-bound: Analysis calculations
- Network-bound: Polygon API calls
- Memory: Caching historical data

### Rate Limiting
- Polygon free tier: 5 calls/minute
- Caching essential for production use
- Consider paid tier for high volume

## Security Considerations

1. **API Keys**: Stored in environment variables
2. **CORS**: Configured for frontend access
3. **Input Validation**: Symbol uppercase, sanitization
4. **Error Handling**: No sensitive data in error messages
5. **Rate Limiting**: Future: Implement per-client limits

## Deployment Architecture

### Development
```
Local Machine
├─ Cargo run (API Server)
├─ Cargo run (Discord Bot)
└─ Docker Compose (Redis)
```

### Production (Recommended)
```
Cloud Provider (AWS/GCP/Azure)
├─ Container: API Server (x N instances)
├─ Container: Discord Bot (x 1 instance)
├─ Redis: Managed service (ElastiCache/Cloud Memorystore)
└─ Load Balancer: Distribute traffic
```

## Future Enhancements

### Short Term
- [ ] WebSocket streaming for real-time data
- [ ] More technical indicators (Fibonacci, Ichimoku)
- [ ] Backtesting framework
- [ ] Database for historical analyses

### Medium Term
- [ ] Machine learning sentiment analysis
- [ ] Portfolio tracking and optimization
- [ ] Custom strategy builder
- [ ] Alerts and notifications

### Long Term
- [ ] Multi-asset support (crypto, forex, options)
- [ ] Social trading features
- [ ] Mobile app
- [ ] Premium tier with advanced features

## Contributing

See individual crate `lib.rs` files for detailed documentation.

Key areas for contribution:
- Additional technical indicators
- Improved fundamental analysis
- ML-based sentiment analysis
- Performance optimizations
- Frontend implementations

## Performance Benchmarks

### Analysis Performance (90-day data, cold cache)

| Component | Time | Notes |
|-----------|------|-------|
| Polygon API calls | 1.5-3s | Network latency |
| Technical Analysis | 5-10ms | CPU-bound |
| Fundamental Analysis | 1-2ms | Lightweight |
| Quant Analysis | 10-15ms | Statistical calculations |
| Sentiment Analysis | 5-10ms | Keyword matching |
| **Total** | **1.5-3s** | Dominated by API calls |

### Caching Performance

| Operation | Time | Notes |
|-----------|------|-------|
| Redis GET | 1-2ms | Local network |
| Redis SET | 1-2ms | Local network |
| Memory GET | <1μs | In-process |
| Memory SET | <1μs | In-process |

## Code Quality

- **Type Safety**: Rust's strong type system
- **Error Handling**: Result types throughout
- **Testing**: Unit tests for each module
- **Documentation**: Inline docs for public APIs
- **Linting**: Clippy for code quality

---

Built with ❤️ in Rust
