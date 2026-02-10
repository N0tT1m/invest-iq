# Stock Suggestions Feature

## Overview

This feature adds intelligent stock screening and suggestions to InvestIQ. It analyzes multiple stocks in parallel and ranks them based on our comprehensive analysis (technical, fundamental, quantitative, and sentiment).

## What Was Added

### 1. Stock Screener Module (`crates/analysis-orchestrator/src/screener.rs`)
- **StockScreener**: Analyzes multiple stocks concurrently using tokio
- **Stock Universes**: Pre-defined sets of stocks (Popular, Tech, Blue Chips, or custom)
- **Filtering**: Configurable filters for minimum confidence and signal strength
- **Ranking**: Composite scoring system that weighs signal strength + confidence
- **Key Highlights**: Extracts notable metrics from each analysis

### 2. API Endpoint (`/api/suggest`)
- **Method**: GET
- **Query Parameters**:
  - `universe`: `popular` (default), `tech`, `bluechip`, or comma-separated symbols
  - `min_confidence`: Minimum confidence score (default: 0.5)
  - `min_signal`: Minimum signal strength -3 to 3 (default: 0)
  - `limit`: Maximum results to return (default: 10)

**Example Requests**:
```bash
# Get top 10 popular stocks
curl "http://localhost:3000/api/suggest"

# Get top tech stocks with high confidence
curl "http://localhost:3000/api/suggest?universe=tech&min_confidence=0.7&limit=5"

# Analyze custom list
curl "http://localhost:3000/api/suggest?universe=AAPL,MSFT,GOOGL&limit=3"
```

**Example Response**:
```json
{
  "success": true,
  "data": {
    "suggestions": [
      {
        "symbol": "NVDA",
        "signal": "Buy",
        "confidence": 0.78,
        "score": 85.2,
        "recommendation": "Buy (confidence: moderate - 78%)",
        "key_highlights": [
          "Technical: Buy (78% conf)",
          "Fundamental: WeakBuy (65% conf)",
          "Strong Sharpe Ratio: 1.45"
        ]
      }
    ],
    "total_analyzed": 30,
    "total_passed_filters": 12,
    "timestamp": "2025-10-06T..."
  }
}
```

### 3. Frontend Dashboard Integration
- **New Section**: "💡 Stock Suggestions" card at the top of the dashboard
- **Universe Selector**: Dropdown to choose stock universe
- **Suggestion Cards**: Display top stocks with:
  - Ranking (#1, #2, etc.)
  - Signal badge (color-coded)
  - Score and confidence
  - Key highlights from analysis
  - Quick "Analyze" button to view full details
- **Interactive**: Click "Analyze" on any suggestion to load it into the main analysis view

## How It Works

1. **Concurrent Analysis**: Uses tokio's JoinSet to analyze all stocks in parallel
2. **Smart Filtering**: Only returns stocks meeting minimum criteria
3. **Composite Scoring**:
   - Signal strength (60%): -3 (StrongSell) to +3 (StrongBuy)
   - Confidence (40%): Model's confidence in the prediction
   - Final score: 0-100
4. **Ranking**: Sorts by composite score, returns top N results

## Stock Universes

### Popular Stocks (30 symbols)
AAPL, MSFT, GOOGL, AMZN, NVDA, TSLA, META, BRK.B, V, JPM, WMT, MA, PG, HD, DIS, NFLX, ADBE, CRM, CSCO, INTC, AMD, PYPL, COST, PEP, TMO, MRK, ABBV, NKE, CVX, MCD

### Tech Stocks (20 symbols)
AAPL, MSFT, GOOGL, AMZN, NVDA, TSLA, META, NFLX, ADBE, CRM, CSCO, INTC, AMD, PYPL, ORCL, IBM, QCOM, NOW, SNOW, ZM

### Blue Chips (20 symbols)
AAPL, MSFT, JPM, JNJ, V, WMT, PG, MA, HD, DIS, CVX, MCD, KO, PEP, CSCO, VZ, INTC, MRK, ABBV, NKE

## Performance Considerations

- **Parallel Processing**: All stocks analyzed concurrently using tokio
- **Rate Limiting**: Be mindful of Polygon API rate limits (consider caching)
- **Typical Performance**:
  - 30 stocks: ~5-10 seconds (depending on API response times)
  - Scales linearly with number of stocks
  - Results are sorted and filtered efficiently in-memory

## Usage

### Via API
```bash
# Start the API server
cargo run --release --bin api-server

# Test the suggestions endpoint
curl "http://localhost:3000/api/suggest?universe=popular&limit=10"
```

### Via Dashboard
1. Start the API server:
   ```bash
   cargo run --release --bin api-server
   ```

2. Start the Dash frontend:
   ```bash
   cd frontend
   python app.py
   ```

3. Open http://localhost:8050
4. Click "Get Suggestions" in the Stock Suggestions section
5. Browse suggestions and click "Analyze" on any stock

## Configuration

### Modify Stock Universes
Edit `crates/analysis-orchestrator/src/screener.rs` to add/remove stocks from universes.

### Adjust Scoring Weights
The composite score calculation is in the `create_suggestion` method:
```rust
let signal_score = (analysis.overall_signal.to_score() + 3) as f64 / 6.0;
let score = (signal_score * 0.6 + analysis.overall_confidence * 0.4) * 100.0;
```

Modify the 0.6/0.4 weights to change how signal strength vs confidence are weighted.

### Filter Defaults
Change defaults in `ScreenerFilters::default()`:
```rust
impl Default for ScreenerFilters {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,      // 50% confidence minimum
            min_signal_strength: 0,    // Neutral or better
            limit: 10,                 // Top 10 results
        }
    }
}
```

## Future Enhancements

- [ ] Add caching for screener results
- [ ] Support more universes (sectors, market cap ranges)
- [ ] Add sorting options (by confidence, signal, volatility, etc.)
- [ ] Real-time updates in dashboard
- [ ] Export suggestions to CSV/JSON
- [ ] Email/Discord notifications for top suggestions
- [ ] Backtesting screener strategy performance
- [ ] Machine learning-based universe selection

## Testing

Build and verify compilation:
```bash
cargo build --release
```

Test the endpoint (requires POLYGON_API_KEY in .env):
```bash
# Run the API server
cargo run --release --bin api-server

# In another terminal
curl "http://localhost:3000/api/suggest?universe=tech&limit=5"
```

## Files Modified/Created

- ✅ `crates/analysis-orchestrator/src/screener.rs` (new)
- ✅ `crates/analysis-orchestrator/src/lib.rs` (updated)
- ✅ `crates/api-server/src/main.rs` (updated)
- ✅ `frontend/app.py` (updated)
- ✅ Fixed validation endpoint type conversion bug
