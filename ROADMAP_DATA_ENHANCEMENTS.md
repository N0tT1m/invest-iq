# InvestIQ — Data Enhancement Roadmap

All items completed.

---

## Completed

- [x] **#1 Surface existing data** — Integrated risk radar, confidence gauge, sentiment velocity components into main dashboard. Enhanced signal card with current price and engine agreement. Expanded all analysis cards with additional metrics.
- [x] **#2 Peer/sector comparison panel** — Side-by-side comparison table and radar chart for sector peers, with color-coded best/worst metrics.
- [x] **#3 Price chart enhancements** — Added SMA 50, VWAP, Fibonacci retracement levels. Added multi-timeframe mini-charts (Daily/Weekly/Monthly).
- [x] **#4 Options Data** — Added `get_options_snapshot()` to polygon-client, `/api/options/:symbol` endpoint computing IV rank, P/C ratio, max pain, unusual activity. Frontend `options_flow.py` with IV gauge, P/C bar, unusual activity table.
- [x] **#5 Insider Trading & Institutional** — Added `get_insider_transactions()` to polygon-client, `/api/insiders/:symbol` endpoint. Frontend `insider_activity.py` with transaction table and net sentiment badges.
- [x] **#6 Earnings Analysis** — `/api/earnings/:symbol` endpoint computing EPS trends, QoQ/YoY growth, beat rate from existing financials. Frontend `earnings_panel.py` with EPS bar chart, revenue trend, growth badges.
- [x] **#7 Short Interest** — `/api/short-interest/:symbol` endpoint with heuristic squeeze risk score (volume spike + momentum + volatility). Frontend `short_interest.py` with squeeze risk gauge.
- [x] **#8 Dividend Analysis** — Added `get_dividends()` to polygon-client using `/v3/reference/dividends`. `/api/dividends/:symbol` endpoint computing yield, growth rate, frequency. Frontend `dividend_panel.py` with history chart and yield badges.
- [x] **#9 Correlation & Portfolio Analytics** — `/api/correlation/:symbol` endpoint fetching SPY/QQQ bars for Pearson correlation and real beta calculation. Frontend `correlation_matrix.py` with correlation bars and diversification score.
- [x] **#10 Economic/Macro Overlay** — `/api/macro/indicators` and `/api/macro/sensitivity/:symbol` endpoints (stub - requires FRED_API_KEY). Frontend `macro_overlay.py` showing setup instructions or indicators when available.
- [x] **#11 Social Sentiment Expansion** — `/api/sentiment/:symbol/social` endpoint (stub - requires REDDIT_CLIENT_ID). Frontend `social_sentiment.py` showing setup instructions or social metrics when available.
- [x] **#12 Pattern Recognition Visuals** — Implemented 4 missing patterns (Piercing, Dark Cloud Cover, Morning Star, Evening Star). Exposed `detected_patterns` array in technical metrics JSON. Frontend annotates patterns on price chart with colored markers. Technical card shows pattern names.

---

## 4. Options Data from Polygon.io

**Effort**: Medium (new backend endpoint + frontend section)

Polygon provides options contracts, implied volatility, and options flow data.

**Backend**:
- Add `get_options_chain()` to `polygon-client` crate using Polygon's `/v3/reference/options/contracts` and `/v3/snapshot/options/{underlyingAsset}` endpoints
- Create `/api/options/:symbol` endpoint in `api-server` returning IV rank, put/call ratio, unusual activity, max pain

**Frontend**:
- New component `components/options_flow.py`
- Display: IV rank/percentile gauge, put/call ratio bar, unusual options activity table, max pain level annotated on price chart

**Key metrics to surface**:
- Implied Volatility Rank (IVR) — where current IV sits relative to its 52-week range
- Put/Call Ratio — bearish/bullish sentiment from options market
- Unusual Options Activity — large volume contracts signaling smart money
- Max Pain — strike price where most options expire worthless

---

## 5. Insider Trading & Institutional Holdings

**Effort**: Medium (new backend endpoint + frontend section)

Polygon provides insider transactions and institutional holdings (13F) data.

**Backend**:
- Add `get_insider_transactions()` to `polygon-client` using `/vX/reference/insiders` endpoint
- Add `get_institutional_holders()` using Polygon's institutional ownership endpoints
- Create `/api/insiders/:symbol` and `/api/institutions/:symbol` endpoints

**Frontend**:
- New component `components/insider_activity.py`
- Display: recent insider buy/sell table with names and amounts, net insider sentiment (buying vs selling), institutional holder changes quarter-over-quarter, ownership concentration pie chart

**Key metrics**:
- Net insider buy/sell ratio (last 3/6/12 months)
- Top institutional holders and position changes
- Insider transaction timeline chart

---

## 6. Earnings Analysis

**Effort**: Medium (leverage existing financials data + new endpoints)

**Backend**:
- Polygon provides earnings data via `/vX/reference/financials` (already partially used in `fundamental-analysis`)
- Create `/api/earnings/:symbol` endpoint aggregating: historical EPS surprise data, revenue growth trend, next earnings date, analyst estimates

**Frontend**:
- New component `components/earnings_panel.py`
- Display: EPS surprise chart (expected vs actual per quarter), revenue growth trend line, next earnings countdown badge, earnings calendar integration

**Key metrics**:
- Historical EPS beat/miss rate
- Average surprise magnitude
- Revenue growth rate (QoQ, YoY)
- Days until next earnings

---

## 7. Short Interest Data

**Effort**: Low-Medium (new endpoint + small frontend panel)

**Backend**:
- Add short interest data from Polygon (if available) or use the existing quantitative metrics
- Create `/api/short-interest/:symbol` endpoint

**Frontend**:
- Add to existing quantitative analysis card or as a standalone badge
- Display: short interest percentage, days to cover, short squeeze risk score

**Key metrics**:
- Short Interest % of Float
- Days to Cover (short interest / avg daily volume)
- Short Squeeze Risk Indicator (high short interest + rising price + high volume)

---

## 8. Dividend Analysis

**Effort**: Low-Medium (leverage existing financial data)

**Backend**:
- Extract dividend data from Polygon's `/v3/reference/dividends` endpoint
- Create `/api/dividends/:symbol` endpoint

**Frontend**:
- New component `components/dividend_panel.py` or extend fundamental card
- Display: current dividend yield, payout ratio, dividend growth rate (5yr CAGR), ex-dividend date countdown, dividend history chart

**Key metrics**:
- Dividend Yield (annual dividend / price)
- Payout Ratio (dividends / earnings)
- Dividend Growth Rate (5-year CAGR)
- Years of Consecutive Increases
- Ex-Dividend Date

---

## 9. Correlation & Portfolio Analytics

**Effort**: High (new analysis engine + frontend component)

**Backend**:
- Create `correlation-analyzer` crate or extend `quant-analysis`
- Calculate: stock-to-stock correlation matrix, stock-to-index (SPY) correlation, beta vs multiple benchmarks, factor exposure (value, momentum, quality, size)
- Create `/api/correlation/:symbol` and `/api/factors/:symbol` endpoints

**Frontend**:
- New component `components/correlation_matrix.py`
- Display: correlation heatmap (vs top 10 stocks), factor exposure radar chart, beta vs multiple indices, diversification score

**Key metrics**:
- Pearson correlation vs SPY, QQQ, sector ETF
- Rolling correlation (30d, 90d, 1yr)
- Factor loadings (Fama-French: market, size, value, momentum, quality)
- Diversification benefit score for portfolio context

---

## 10. Economic/Macro Overlay

**Effort**: High (new data source + analysis engine)

**Backend**:
- Integrate FRED API (Federal Reserve Economic Data) for macro indicators
- Create `macro-analyzer` crate
- Create `/api/macro/indicators` and `/api/macro/sensitivity/:symbol` endpoints

**Frontend**:
- New component `components/macro_overlay.py`
- Display: interest rate sensitivity score, inflation correlation, economic cycle positioning, sector rotation model

**Key metrics**:
- Interest Rate Sensitivity (correlation with 10Y Treasury yield)
- Inflation Beta (correlation with CPI changes)
- Economic Cycle Phase (expansion, peak, contraction, trough)
- Sector Rotation Recommendation based on current cycle

**Data sources**:
- Federal Funds Rate, 10Y Treasury, CPI, unemployment, GDP growth
- FRED API is free with registration

---

## 11. Social Sentiment Expansion

**Effort**: Medium-High (new data sources + NLP pipeline)

Currently only news-based sentiment. Expand to social media.

**Backend**:
- Integrate Reddit API (r/wallstreetbets, r/stocks, r/investing) for social mentions
- Optionally integrate X/Twitter API for financial influencer tracking
- Extend `sentiment-analysis` crate with social sentiment scoring
- Create `/api/sentiment/:symbol/social` endpoint

**Frontend**:
- Extend existing sentiment card or new `components/social_sentiment.py`
- Display: Reddit mention volume chart, social sentiment score vs news sentiment, trending tickers list, sentiment divergence alerts (when social and news disagree)

**Key metrics**:
- Reddit Mention Volume (24h, 7d trend)
- Social Sentiment Score (-100 to +100)
- Sentiment Divergence (social vs institutional news)
- Buzz Score (abnormal mention volume vs baseline)

---

## 12. Pattern Recognition Visuals

**Effort**: Medium (frontend visualization of existing backend data)

The `technical-analysis` crate already detects 11 candlestick patterns but only reports the count. Surface the actual patterns visually.

**Backend**:
- Modify `technical-analysis` to return pattern details: which patterns, at which bar indices, pattern type (bullish/bearish/neutral)
- Update the `metrics` JSON to include `detected_patterns: [{name, index, type, description}]`

**Frontend**:
- Annotate detected patterns directly on the price chart with markers and labels
- Add a pattern legend/summary below the chart
- Display: pattern name annotations on chart at detection points, bullish patterns in green / bearish in red, pattern description tooltips on hover

**Patterns already detected in backend** (`technical-analysis/src/analyzer.rs`):
- Doji, Hammer, Inverted Hammer, Shooting Star
- Bullish/Bearish Engulfing, Piercing, Dark Cloud Cover
- Morning Star, Evening Star
- Three White Soldiers, Three Black Crows

---

## Priority Recommendation

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| 1 | #12 Pattern Recognition | High | Medium |
| 2 | #6 Earnings Analysis | High | Medium |
| 3 | #4 Options Data | High | Medium |
| 4 | #8 Dividend Analysis | Medium | Low |
| 5 | #7 Short Interest | Medium | Low |
| 6 | #5 Insider/Institutional | Medium | Medium |
| 7 | #9 Correlation Analytics | High | High |
| 8 | #11 Social Sentiment | Medium | High |
| 9 | #10 Macro Overlay | Medium | High |

Pattern recognition (#12) is highest priority because the backend already does the work — it just needs to expose the data and the frontend needs to render it. Earnings (#6) and options (#4) are next because they add the most decision-relevant data using Polygon's existing API.
