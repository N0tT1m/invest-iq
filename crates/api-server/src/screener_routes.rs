use analysis_core::{Bar, Timeframe};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task::JoinSet;

use crate::{get_cached_etf_bars, ApiResponse, AppError, AppState};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FilterCriterion {
    pub field: String,
    pub operator: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    Gt,
    Lt,
    Gte,
    Lte,
    Eq,
    Between,
    In,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ScreenRequest {
    #[serde(default)]
    pub filters: Vec<FilterCriterion>,
    #[serde(default = "default_universe")]
    pub universe: String,
    #[serde(default = "default_sort")]
    pub sort_by: String,
    #[serde(default)]
    pub sort_desc: Option<bool>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// "quick" (bars-only, fast) or "full" (full orchestrator analysis)
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_universe() -> String {
    "popular".into()
}
fn default_sort() -> String {
    "score".into()
}
fn default_limit() -> usize {
    20
}
fn default_mode() -> String {
    "quick".into()
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ScreenResult {
    pub results: Vec<ScreenedStock>,
    pub total_scanned: usize,
    pub total_matched: usize,
    pub mode: String,
    pub filters_applied: usize,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ScreenedStock {
    pub symbol: String,
    pub price: f64,
    pub rsi: Option<f64>,
    pub sma_20_pct: Option<f64>,
    pub sma_50_pct: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub momentum_5d: Option<f64>,
    pub signal_score: f64,
    pub confidence: f64,
    pub tags: Vec<String>,
    // Full-mode fields (None in quick mode)
    pub pe_ratio: Option<f64>,
    pub revenue_growth: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub sentiment_score: Option<f64>,
    pub overall_signal: Option<String>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ScreenerPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filters: Vec<FilterCriterion>,
}

// ---------------------------------------------------------------------------
// Universe resolution
// ---------------------------------------------------------------------------

const SCREENER_UNIVERSE: &[&str] = &[
    // Technology
    "AAPL", "MSFT", "NVDA", "AVGO", "AMD", "CRM", "ORCL", "ADBE", "INTC", "QCOM",
    // Communication Services
    "GOOGL", "META", "NFLX", "DIS", // Consumer Discretionary
    "AMZN", "TSLA", "NKE", "SBUX", "MCD", "HD", // Financials
    "JPM", "V", "GS", "BAC", "MA", "BRK.B", // Healthcare
    "UNH", "JNJ", "LLY", "PFE", "ABBV", "MRK", "TMO", // Energy
    "XOM", "CVX", "COP", // Consumer Staples
    "PG", "KO", "COST", "PEP", "WMT", // Industrials
    "CAT", "BA", "UPS", "GE", // Materials & Utilities
    "LIN", "NEE", "SO",
];

fn resolve_universe(name: &str) -> Vec<String> {
    match name {
        "popular" => SCREENER_UNIVERSE.iter().map(|s| s.to_string()).collect(),
        "tech" => vec![
            "AAPL", "MSFT", "NVDA", "AVGO", "AMD", "CRM", "ORCL", "ADBE", "INTC", "QCOM", "NOW",
            "SNOW", "PLTR", "NET", "PANW",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        "bluechip" => vec![
            "AAPL", "MSFT", "JPM", "JNJ", "V", "WMT", "PG", "MA", "HD", "DIS", "CVX", "MCD", "KO",
            "PEP", "MRK", "ABBV", "NKE",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        "dividend" => vec![
            "JNJ", "PG", "KO", "PEP", "MCD", "ABBV", "XOM", "CVX", "MRK", "HD", "WMT", "SO", "NEE",
            "LIN", "V", "MA",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        custom => custom
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Quick scan (bars-only)
// ---------------------------------------------------------------------------

struct QuickMetrics {
    price: f64,
    rsi: Option<f64>,
    sma_20_pct: Option<f64>,
    sma_50_pct: Option<f64>,
    volume_ratio: Option<f64>,
    momentum_5d: Option<f64>,
    signal_score: f64,
    confidence: f64,
    tags: Vec<String>,
}

fn compute_quick_metrics(bars: &[Bar]) -> Option<QuickMetrics> {
    if bars.len() < 21 {
        return None;
    }
    let len = bars.len();
    let current = bars.last()?.close;

    // SMA-20
    let sma_20: f64 = bars[len - 20..].iter().map(|b| b.close).sum::<f64>() / 20.0;
    let sma_20_pct = (current - sma_20) / sma_20 * 100.0;

    // SMA-50
    let (sma_50, sma_50_pct) = if len >= 50 {
        let s = bars[len - 50..].iter().map(|b| b.close).sum::<f64>() / 50.0;
        (s, Some((current - s) / s * 100.0))
    } else {
        (sma_20, None)
    };

    // RSI-14
    let rsi_period = 14.min(len - 1);
    let (mut gains, mut losses) = (0.0_f64, 0.0_f64);
    for i in (len - rsi_period)..len {
        let change = bars[i].close - bars[i - 1].close;
        if change > 0.0 {
            gains += change;
        } else {
            losses += change.abs();
        }
    }
    let avg_gain = gains / rsi_period as f64;
    let avg_loss = losses / rsi_period as f64;
    let rsi = if avg_loss == 0.0 {
        100.0
    } else {
        100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
    };

    // 5-day momentum
    let momentum_5d = if len >= 6 {
        Some((current - bars[len - 6].close) / bars[len - 6].close * 100.0)
    } else {
        None
    };

    // Volume ratio (5d avg / 20d avg)
    let volume_ratio = if len >= 20 {
        let avg_5d: f64 = bars[len - 5..].iter().map(|b| b.volume).sum::<f64>() / 5.0;
        let avg_20d: f64 = bars[len - 20..].iter().map(|b| b.volume).sum::<f64>() / 20.0;
        if avg_20d > 0.0 {
            Some(avg_5d / avg_20d)
        } else {
            None
        }
    } else {
        None
    };

    // Tags
    let mut tags = Vec::new();
    if sma_20_pct > 3.0 && sma_50_pct.unwrap_or(0.0) > 3.0 {
        tags.push("Uptrend".into());
    } else if sma_20_pct < -3.0 {
        tags.push("Downtrend".into());
    }
    if rsi > 70.0 {
        tags.push("Overbought".into());
    }
    if rsi < 30.0 {
        tags.push("Oversold".into());
    }
    if volume_ratio.unwrap_or(1.0) > 1.5 {
        tags.push("High Volume".into());
    }
    if let Some(m) = momentum_5d {
        if m > 5.0 {
            tags.push("Momentum".into());
        }
    }

    // Golden/death cross
    if len >= 50 {
        let prev_sma_20: f64 = bars[len - 21..len - 1].iter().map(|b| b.close).sum::<f64>() / 20.0;
        let prev_sma_50: f64 = bars[len - 51..len - 1].iter().map(|b| b.close).sum::<f64>() / 50.0;
        if sma_20 > sma_50 && prev_sma_20 <= prev_sma_50 {
            tags.push("Golden Cross".into());
        } else if sma_20 < sma_50 && prev_sma_20 >= prev_sma_50 {
            tags.push("Death Cross".into());
        }
    }

    // Composite score
    let trend_score =
        ((sma_20_pct + sma_50_pct.unwrap_or(sma_20_pct)) / 2.0 / 10.0 + 0.5).clamp(0.0, 1.0);
    let rsi_score = if rsi > 70.0 {
        0.3
    } else if rsi > 50.0 {
        0.7
    } else if rsi > 30.0 {
        0.4
    } else {
        0.2
    };
    let signal_score = trend_score * 0.6 + rsi_score * 0.4;
    let confidence = (sma_20_pct.abs() / 8.0).clamp(0.3, 0.85);

    Some(QuickMetrics {
        price: current,
        rsi: Some(rsi),
        sma_20_pct: Some(sma_20_pct),
        sma_50_pct,
        volume_ratio,
        momentum_5d,
        signal_score,
        confidence,
        tags,
    })
}

// ---------------------------------------------------------------------------
// Filter evaluation
// ---------------------------------------------------------------------------

fn evaluate_filter(stock: &ScreenedStock, filter: &FilterCriterion) -> bool {
    let field_val = match filter.field.as_str() {
        "price" => Some(stock.price),
        "rsi" => stock.rsi,
        "sma_20_pct" => stock.sma_20_pct,
        "sma_50_pct" => stock.sma_50_pct,
        "volume_ratio" => stock.volume_ratio,
        "momentum_5d" => stock.momentum_5d,
        "signal_score" => Some(stock.signal_score),
        "confidence" => Some(stock.confidence),
        "pe_ratio" => stock.pe_ratio,
        "revenue_growth" => stock.revenue_growth,
        "sharpe_ratio" => stock.sharpe_ratio,
        "sentiment_score" => stock.sentiment_score,
        _ => return true, // Unknown field: pass through
    };

    let field_val = match field_val {
        Some(v) => v,
        None => return false, // Field not available: filter out
    };

    match filter.operator {
        FilterOperator::Gt => filter.value.as_f64().is_some_and(|v| field_val > v),
        FilterOperator::Lt => filter.value.as_f64().is_some_and(|v| field_val < v),
        FilterOperator::Gte => filter.value.as_f64().is_some_and(|v| field_val >= v),
        FilterOperator::Lte => filter.value.as_f64().is_some_and(|v| field_val <= v),
        FilterOperator::Eq => filter
            .value
            .as_f64()
            .is_some_and(|v| (field_val - v).abs() < 0.001),
        FilterOperator::Between => {
            if let Some(arr) = filter.value.as_array() {
                if arr.len() == 2 {
                    let lo = arr[0].as_f64().unwrap_or(f64::MIN);
                    let hi = arr[1].as_f64().unwrap_or(f64::MAX);
                    field_val >= lo && field_val <= hi
                } else {
                    true
                }
            } else {
                true
            }
        }
        FilterOperator::In => {
            if let Some(arr) = filter.value.as_array() {
                arr.iter()
                    .any(|v| v.as_f64().is_some_and(|vv| (field_val - vv).abs() < 0.001))
            } else {
                true
            }
        }
    }
}

fn sort_stocks(stocks: &mut [ScreenedStock], sort_by: &str, desc: bool) {
    stocks.sort_by(|a, b| {
        let val_a = get_sort_value(a, sort_by);
        let val_b = get_sort_value(b, sort_by);
        let cmp = val_a
            .partial_cmp(&val_b)
            .unwrap_or(std::cmp::Ordering::Equal);
        if desc {
            cmp.reverse()
        } else {
            cmp
        }
    });
}

fn get_sort_value(stock: &ScreenedStock, field: &str) -> f64 {
    match field {
        "price" => stock.price,
        "rsi" => stock.rsi.unwrap_or(50.0),
        "signal_score" | "score" => stock.signal_score,
        "confidence" => stock.confidence,
        "momentum_5d" | "momentum" => stock.momentum_5d.unwrap_or(0.0),
        "volume_ratio" | "volume" => stock.volume_ratio.unwrap_or(1.0),
        "pe_ratio" => stock.pe_ratio.unwrap_or(999.0),
        "sharpe_ratio" | "sharpe" => stock.sharpe_ratio.unwrap_or(0.0),
        "sentiment_score" | "sentiment" => stock.sentiment_score.unwrap_or(0.5),
        _ => stock.signal_score,
    }
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

fn get_presets() -> Vec<ScreenerPreset> {
    vec![
        ScreenerPreset {
            id: "oversold_bounce".into(),
            name: "Oversold Bounce".into(),
            description: "Stocks with RSI below 30 that may be due for a bounce".into(),
            filters: vec![FilterCriterion {
                field: "rsi".into(),
                operator: FilterOperator::Lt,
                value: serde_json::json!(30.0),
            }],
        },
        ScreenerPreset {
            id: "momentum_leaders".into(),
            name: "Momentum Leaders".into(),
            description: "Strong uptrend with high 5-day momentum and volume confirmation".into(),
            filters: vec![
                FilterCriterion {
                    field: "momentum_5d".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(3.0),
                },
                FilterCriterion {
                    field: "sma_20_pct".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(2.0),
                },
                FilterCriterion {
                    field: "volume_ratio".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(1.2),
                },
            ],
        },
        ScreenerPreset {
            id: "high_confidence".into(),
            name: "High Confidence".into(),
            description: "Stocks where technical signals are strongest".into(),
            filters: vec![
                FilterCriterion {
                    field: "confidence".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(0.7),
                },
                FilterCriterion {
                    field: "signal_score".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(0.65),
                },
            ],
        },
        ScreenerPreset {
            id: "overbought_warning".into(),
            name: "Overbought Warning".into(),
            description: "Stocks with RSI above 70 that may be extended".into(),
            filters: vec![FilterCriterion {
                field: "rsi".into(),
                operator: FilterOperator::Gt,
                value: serde_json::json!(70.0),
            }],
        },
        ScreenerPreset {
            id: "volume_breakout".into(),
            name: "Volume Breakout".into(),
            description: "Unusual volume activity with positive momentum".into(),
            filters: vec![
                FilterCriterion {
                    field: "volume_ratio".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(2.0),
                },
                FilterCriterion {
                    field: "momentum_5d".into(),
                    operator: FilterOperator::Gt,
                    value: serde_json::json!(0.0),
                },
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/api/screener/scan",
    request_body = ScreenRequest,
    responses((status = 200, description = "Screener results with filtered stocks")),
    tag = "Screener"
)]
async fn scan_stocks(
    State(state): State<AppState>,
    Json(req): Json<ScreenRequest>,
) -> Result<Json<ApiResponse<ScreenResult>>, AppError> {
    let symbols = resolve_universe(&req.universe);
    let total_scanned = symbols.len();

    tracing::info!(
        "Screener scan: {} symbols, {} filters, mode={}",
        total_scanned,
        req.filters.len(),
        req.mode
    );

    let mut screened: Vec<ScreenedStock> = if req.mode == "full" {
        // Full mode: orchestrator analysis (slower, richer data)
        let mut tasks = JoinSet::new();
        let orch = Arc::clone(&state.orchestrator);
        for symbol in symbols {
            let orch = Arc::clone(&orch);
            tasks.spawn(async move {
                let result = orch.analyze(&symbol, Timeframe::Day1, 365).await;
                (symbol, result)
            });
        }

        let mut results = Vec::new();
        while let Some(join_result) = tasks.join_next().await {
            if let Ok((symbol, Ok(analysis))) = join_result {
                let tech = &analysis.technical;
                let fund = &analysis.fundamental;
                let quant = &analysis.quantitative;
                let sent = &analysis.sentiment;

                let rsi = tech
                    .as_ref()
                    .and_then(|t| t.metrics.get("rsi"))
                    .and_then(|v| v.as_f64());
                let pe_ratio = fund
                    .as_ref()
                    .and_then(|f| f.metrics.get("pe_ratio"))
                    .and_then(|v| v.as_f64());
                let revenue_growth = fund
                    .as_ref()
                    .and_then(|f| f.metrics.get("revenue_growth"))
                    .and_then(|v| v.as_f64());
                let sharpe = quant
                    .as_ref()
                    .and_then(|q| q.metrics.get("sharpe_ratio"))
                    .and_then(|v| v.as_f64());
                let sentiment_score = sent.as_ref().map(|s| s.confidence);

                let signal_score = (analysis.overall_signal.to_score() + 100) as f64 / 200.0;
                let price = analysis.current_price.unwrap_or(0.0);

                results.push(ScreenedStock {
                    symbol,
                    price,
                    rsi,
                    sma_20_pct: tech
                        .as_ref()
                        .and_then(|t| t.metrics.get("sma_20"))
                        .and_then(|v| v.as_f64())
                        .map(|sma| {
                            if sma > 0.0 {
                                (price - sma) / sma * 100.0
                            } else {
                                0.0
                            }
                        }),
                    sma_50_pct: tech
                        .as_ref()
                        .and_then(|t| t.metrics.get("sma_50"))
                        .and_then(|v| v.as_f64())
                        .map(|sma| {
                            if sma > 0.0 {
                                (price - sma) / sma * 100.0
                            } else {
                                0.0
                            }
                        }),
                    volume_ratio: None,
                    momentum_5d: None,
                    signal_score,
                    confidence: analysis.overall_confidence,
                    tags: vec![],
                    pe_ratio,
                    revenue_growth,
                    sharpe_ratio: sharpe,
                    sentiment_score,
                    overall_signal: Some(format!("{:?}", analysis.overall_signal)),
                    recommendation: Some(analysis.recommendation),
                });
            }
        }
        results
    } else {
        // Quick mode: bars-only via cached ETF bars helper (fast, technical-only)
        let mut results = Vec::new();
        for symbol in &symbols {
            let bars = get_cached_etf_bars(&state, symbol, 90, 15).await;
            if bars.is_empty() {
                continue;
            }
            if let Some(metrics) = compute_quick_metrics(&bars) {
                results.push(ScreenedStock {
                    symbol: symbol.clone(),
                    price: metrics.price,
                    rsi: metrics.rsi,
                    sma_20_pct: metrics.sma_20_pct,
                    sma_50_pct: metrics.sma_50_pct,
                    volume_ratio: metrics.volume_ratio,
                    momentum_5d: metrics.momentum_5d,
                    signal_score: metrics.signal_score,
                    confidence: metrics.confidence,
                    tags: metrics.tags,
                    pe_ratio: None,
                    revenue_growth: None,
                    sharpe_ratio: None,
                    sentiment_score: None,
                    overall_signal: None,
                    recommendation: None,
                });
            }
        }
        results
    };

    // Apply filters
    let filters_applied = req.filters.len();
    screened.retain(|stock| req.filters.iter().all(|f| evaluate_filter(stock, f)));
    let total_matched = screened.len();

    // Sort
    let desc = req.sort_desc.unwrap_or(true);
    sort_stocks(&mut screened, &req.sort_by, desc);

    // Limit
    screened.truncate(req.limit);

    Ok(Json(ApiResponse::success(ScreenResult {
        results: screened,
        total_scanned,
        total_matched,
        mode: req.mode,
        filters_applied,
    })))
}

#[utoipa::path(
    get,
    path = "/api/screener/presets",
    responses((status = 200, description = "Available screener presets")),
    tag = "Screener"
)]
async fn get_screener_presets() -> Json<ApiResponse<Vec<ScreenerPreset>>> {
    Json(ApiResponse::success(get_presets()))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn screener_routes() -> Router<AppState> {
    Router::new()
        .route("/api/screener/scan", post(scan_stocks))
        .route("/api/screener/presets", get(get_screener_presets))
}
