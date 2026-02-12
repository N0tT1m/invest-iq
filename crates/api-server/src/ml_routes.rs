use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use ml_client::price_predictor::PriceData;
use ml_client::MLError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

/// Map MLError to AppError with appropriate status codes.
fn ml_err(context: &str, e: MLError) -> AppError {
    match e {
        MLError::ServiceUnavailable(_) | MLError::ModelNotLoaded => AppError::with_status(
            StatusCode::SERVICE_UNAVAILABLE,
            anyhow::anyhow!("{context}: {e}"),
        ),
        other => AppError::with_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            anyhow::anyhow!("{context}: {other}"),
        ),
    }
}

// ─── Query params ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ForecastQuery {
    #[serde(default = "default_horizon")]
    pub horizon: i32,
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_horizon() -> i32 {
    5
}

fn default_days() -> i64 {
    750 // ~512 trading days needed for PatchTST context_length
}

// ─── Response types ─────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct TradeSignalResponse {
    pub symbol: String,
    pub probability: f64,
    pub expected_return: f64,
    pub recommendation: String,
    pub features_used: HashMap<String, f64>,
    pub backend: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SentimentArticle {
    pub headline: String,
    pub label: String,
    pub positive: f64,
    pub negative: f64,
    pub neutral: f64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MLSentimentResponse {
    pub symbol: String,
    pub overall_sentiment: String,
    pub score: f64,
    pub confidence: f64,
    pub positive_ratio: f64,
    pub negative_ratio: f64,
    pub neutral_ratio: f64,
    pub article_count: usize,
    pub articles: Vec<SentimentArticle>,
    pub backend: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PriceForecastResponse {
    pub symbol: String,
    pub direction: String,
    pub confidence: f64,
    pub probabilities: HashMap<String, f64>,
    pub predicted_prices: Vec<f64>,
    pub horizon_steps: i32,
    pub current_price: Option<f64>,
    pub backend: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct StrategyWeightEntry {
    pub name: String,
    pub alpha: f64,
    pub beta: f64,
    pub win_rate: f64,
    pub total_samples: i32,
    pub weight: f64,
    pub credible_interval: Option<(f64, f64)>,
    pub recommendation: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct StrategyWeightsResponse {
    pub weights: HashMap<String, f64>,
    pub strategies: Vec<StrategyWeightEntry>,
    pub backend: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct EngineCalibrationEntry {
    pub raw: f64,
    pub calibrated: f64,
    pub tier: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CalibrationResponse {
    pub symbol: String,
    pub engines: HashMap<String, EngineCalibrationEntry>,
    pub regime: String,
    pub backend: String,
}

// ─── Router ─────────────────────────────────────────────────────────────────

pub fn ml_routes() -> Router<AppState> {
    Router::new()
        .route("/api/ml/trade-signal/:symbol", get(trade_signal))
        .route("/api/ml/sentiment/:symbol", get(ml_sentiment))
        .route("/api/ml/price-forecast/:symbol", get(price_forecast))
        .route("/api/ml/strategy-weights", get(strategy_weights))
        .route("/api/ml/calibration/:symbol", get(ml_calibration))
        .route("/api/ml/earnings-nlp/:symbol", get(earnings_nlp))
        .route("/api/ml/social-sentiment/:symbol", get(social_sentiment))
}

// ─── Handlers ───────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/ml/trade-signal/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "ML trade signal prediction with probability and features")),
    tag = "Analysis"
)]
async fn trade_signal(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<TradeSignalResponse>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();

    // Get analysis to build feature vector
    let analysis = get_default_analysis(&state, &symbol).await?;

    let mut features = HashMap::new();

    // Engine scores + confidences
    if let Some(tech) = &analysis.technical {
        features.insert("technical_score".into(), tech.signal.to_score() as f64);
        features.insert("technical_confidence".into(), tech.confidence);
        if let Some(rsi) = tech.metrics.get("rsi").and_then(|v| v.as_f64()) {
            features.insert("rsi".into(), rsi);
        }
        if let Some(bb) = tech.metrics.get("bb_percent_b").and_then(|v| v.as_f64()) {
            features.insert("bb_percent_b".into(), bb);
        }
        if let Some(adx) = tech.metrics.get("adx").and_then(|v| v.as_f64()) {
            features.insert("adx".into(), adx);
        }
        // SMA ratio
        let sma20 = tech
            .metrics
            .get("sma_20")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let sma50 = tech
            .metrics
            .get("sma_50")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        if sma50 > 0.0 {
            features.insert("sma_20_vs_50".into(), sma20 / sma50);
        }
    }

    if let Some(fund) = &analysis.fundamental {
        features.insert("fundamental_score".into(), fund.signal.to_score() as f64);
        features.insert("fundamental_confidence".into(), fund.confidence);
        if let Some(pe) = fund.metrics.get("pe_ratio").and_then(|v| v.as_f64()) {
            features.insert("pe_ratio".into(), pe);
        }
        if let Some(de) = fund.metrics.get("debt_to_equity").and_then(|v| v.as_f64()) {
            features.insert("debt_to_equity".into(), de);
        }
        if let Some(rg) = fund.metrics.get("revenue_growth").and_then(|v| v.as_f64()) {
            features.insert("revenue_growth".into(), rg);
        }
        if let Some(roic) = fund.metrics.get("roic").and_then(|v| v.as_f64()) {
            features.insert("roic".into(), roic);
        }
    }

    if let Some(quant) = &analysis.quantitative {
        features.insert("quant_score".into(), quant.signal.to_score() as f64);
        features.insert("quant_confidence".into(), quant.confidence);
        if let Some(sr) = quant.metrics.get("sharpe_ratio").and_then(|v| v.as_f64()) {
            features.insert("sharpe_ratio".into(), sr);
        }
        if let Some(vol) = quant.metrics.get("volatility").and_then(|v| v.as_f64()) {
            features.insert("volatility".into(), vol);
        }
        if let Some(md) = quant.metrics.get("max_drawdown").and_then(|v| v.as_f64()) {
            features.insert("max_drawdown".into(), md);
        }
        if let Some(beta) = quant.metrics.get("beta").and_then(|v| v.as_f64()) {
            features.insert("beta".into(), beta);
        }
    }

    if let Some(sent) = &analysis.sentiment {
        features.insert("sentiment_score".into(), sent.signal.to_score() as f64);
        features.insert("sentiment_confidence".into(), sent.confidence);
        if let Some(ns) = sent
            .metrics
            .get("normalized_score")
            .and_then(|v| v.as_f64())
        {
            features.insert("normalized_sentiment_score".into(), ns);
        }
        if let Some(ac) = sent.metrics.get("article_count").and_then(|v| v.as_f64()) {
            features.insert("article_count".into(), ac);
        }
        if let Some(dr) = sent
            .metrics
            .get("direct_mention_ratio")
            .and_then(|v| v.as_f64())
        {
            features.insert("direct_mention_ratio".into(), dr);
        }
    }

    // Market context
    features.insert("market_regime_encoded".into(), 4.0); // default: normal
    features.insert("inter_engine_agreement".into(), 0.0);
    features.insert("vix_proxy".into(), 0.0);

    let prediction = provider
        .predict_trade(&features)
        .await
        .map_err(|e| ml_err("ML trade signal unavailable", e))?;

    Ok(Json(ApiResponse::success(TradeSignalResponse {
        symbol,
        probability: prediction.probability,
        expected_return: prediction.expected_return,
        recommendation: prediction.recommendation,
        features_used: features,
        backend: provider.backend_name().into(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/ml/sentiment/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "ML-powered news sentiment analysis")),
    tag = "Sentiment"
)]
async fn ml_sentiment(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<MLSentimentResponse>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();

    // Fetch news headlines
    let news = state
        .orchestrator
        .get_news(&symbol, 20)
        .await
        .unwrap_or_default();

    if news.is_empty() {
        return Ok(Json(ApiResponse::success(MLSentimentResponse {
            symbol,
            overall_sentiment: "neutral".into(),
            score: 0.0,
            confidence: 0.0,
            positive_ratio: 0.0,
            negative_ratio: 0.0,
            neutral_ratio: 0.0,
            article_count: 0,
            articles: vec![],
            backend: provider.backend_name().into(),
        })));
    }

    let headlines: Vec<String> = news.iter().map(|n| n.title.clone()).collect();
    let descriptions: Vec<String> = news
        .iter()
        .map(|n| n.description.clone().unwrap_or_default())
        .collect();

    // Get aggregate sentiment
    let agg = provider
        .analyze_news(headlines.clone(), Some(descriptions))
        .await
        .map_err(|e| ml_err("Sentiment model unavailable", e))?;

    // Get per-article breakdown
    let per_article = provider.predict_sentiment(headlines.clone()).await.ok();

    let articles: Vec<SentimentArticle> = match per_article {
        Some(resp) => resp
            .predictions
            .into_iter()
            .zip(headlines.iter())
            .map(|(pred, headline)| SentimentArticle {
                headline: headline.clone(),
                label: pred.label,
                positive: pred.positive,
                negative: pred.negative,
                neutral: pred.neutral,
            })
            .collect(),
        None => vec![],
    };

    Ok(Json(ApiResponse::success(MLSentimentResponse {
        symbol,
        overall_sentiment: agg.overall_sentiment,
        score: agg.score,
        confidence: agg.confidence,
        positive_ratio: agg.positive_ratio,
        negative_ratio: agg.negative_ratio,
        neutral_ratio: agg.neutral_ratio,
        article_count: agg.article_count,
        articles,
        backend: provider.backend_name().into(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/ml/price-forecast/{symbol}",
    params(
        ("symbol" = String, Path, description = "Stock ticker symbol"),
        ("horizon" = i32, Query, description = "Prediction horizon in steps"),
        ("days" = i64, Query, description = "Historical bars to fetch (default 750)")
    ),
    responses((status = 200, description = "Price direction forecast with confidence")),
    tag = "Analysis"
)]
async fn price_forecast(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(params): Query<ForecastQuery>,
) -> Result<Json<ApiResponse<PriceForecastResponse>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();

    // Fetch historical bars
    let bars = state
        .orchestrator
        .get_bars(&symbol, analysis_core::Timeframe::Day1, params.days)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch bars: {e}"))?;

    if bars.len() < 30 {
        return Err(anyhow::anyhow!("Insufficient history ({} bars, need 30+)", bars.len()).into());
    }

    let current_price = bars.last().map(|b| b.close);

    let history: Vec<PriceData> = bars
        .iter()
        .map(|b| PriceData {
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
            vwap: b.vwap,
        })
        .collect();

    let prediction = provider
        .predict_price(&symbol, history, params.horizon)
        .await
        .map_err(|e| ml_err("Price predictor unavailable", e))?;

    Ok(Json(ApiResponse::success(PriceForecastResponse {
        symbol,
        direction: prediction.direction,
        confidence: prediction.confidence,
        probabilities: prediction.probabilities,
        predicted_prices: prediction.predicted_prices,
        horizon_steps: prediction.horizon_steps,
        current_price,
        backend: provider.backend_name().into(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/ml/strategy-weights",
    responses((status = 200, description = "Bayesian strategy weights and recommendations")),
    tag = "Strategies"
)]
async fn strategy_weights(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<StrategyWeightsResponse>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let weights = provider
        .get_strategy_weights(true)
        .await
        .unwrap_or_default();

    let stats = provider.get_all_strategy_stats().await.unwrap_or_default();

    let mut strategies: Vec<StrategyWeightEntry> = stats
        .into_iter()
        .map(|s| {
            let _backend = provider.backend_name();
            StrategyWeightEntry {
                name: s.strategy_name,
                alpha: s.alpha,
                beta: s.beta,
                win_rate: s.win_rate,
                total_samples: s.total_samples,
                weight: s.weight,
                credible_interval: s.credible_interval,
                recommendation: None,
            }
        })
        .collect();

    // Fetch recommendations for each strategy (best-effort)
    for entry in &mut strategies {
        if let Ok(rec) = provider.get_recommendation(&entry.name).await {
            entry.recommendation = Some(rec.reason);
        }
    }

    Ok(Json(ApiResponse::success(StrategyWeightsResponse {
        weights,
        strategies,
        backend: provider.backend_name().into(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/ml/calibration/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Engine confidence calibration per regime")),
    tag = "Analysis"
)]
async fn ml_calibration(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<CalibrationResponse>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();

    // Get analysis to extract engine confidences
    let analysis = get_default_analysis(&state, &symbol).await?;

    let mut raw_confidences = HashMap::new();
    if let Some(tech) = &analysis.technical {
        raw_confidences.insert("technical".into(), tech.confidence);
    }
    if let Some(fund) = &analysis.fundamental {
        raw_confidences.insert("fundamental".into(), fund.confidence);
    }
    if let Some(quant) = &analysis.quantitative {
        raw_confidences.insert("quantitative".into(), quant.confidence);
    }
    if let Some(sent) = &analysis.sentiment {
        raw_confidences.insert("sentiment".into(), sent.confidence);
    }

    let regime = analysis
        .market_regime
        .as_deref()
        .unwrap_or("normal")
        .to_string();

    let calibrated = provider
        .batch_calibrate(&raw_confidences, &regime)
        .await
        .map_err(|e| ml_err("Calibration model unavailable", e))?;

    let mut engines = HashMap::new();
    for (engine, raw) in &raw_confidences {
        let entry = calibrated.get(engine);
        engines.insert(
            engine.clone(),
            EngineCalibrationEntry {
                raw: *raw,
                calibrated: entry.map(|c| c.calibrated_confidence).unwrap_or(*raw),
                tier: entry
                    .map(|c| c.reliability_tier.clone())
                    .unwrap_or_else(|| "unknown".into()),
            },
        );
    }

    Ok(Json(ApiResponse::success(CalibrationResponse {
        symbol,
        engines,
        regime,
        backend: provider.backend_name().into(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/ml/earnings-nlp/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Earnings transcript NLP analysis")),
    tag = "Analysis"
)]
async fn earnings_nlp(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();
    let result = provider
        .analyze_earnings(&symbol)
        .await
        .map_err(|e| ml_err("Earnings NLP unavailable", e))?;

    Ok(Json(ApiResponse::success(result)))
}

#[utoipa::path(
    get,
    path = "/api/ml/social-sentiment/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Social media sentiment analysis")),
    tag = "Sentiment"
)]
async fn social_sentiment(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let provider = state
        .ml_provider
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("ML provider not configured"))?;

    let symbol = symbol.to_uppercase();
    let result = provider
        .get_social_sentiment(&symbol)
        .await
        .map_err(|e| ml_err("Social sentiment unavailable", e))?;

    Ok(Json(ApiResponse::success(result)))
}
