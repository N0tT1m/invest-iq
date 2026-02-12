use analysis_core::{
    adaptive, AnalysisError, AnalysisResult, AnalystConsensusData, Bar, Financials, NewsArticle,
    SentimentAnalyzer, SignalStrength, Timeframe, UnifiedAnalysis,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use fundamental_analysis::FundamentalAnalysisEngine;
use ml_client::SignalModelsClient;
use polygon_client::{PolygonClient, TickerDetails};
use quant_analysis::QuantAnalysisEngine;
use sentiment_analysis::SentimentAnalysisEngine;
use serde_json::json;
use std::collections::HashMap;
use technical_analysis::TechnicalAnalysisEngine;

pub mod screener;
pub use screener::{
    ScreenerFilters, ScreenerResult, StockScreener, StockSuggestion, StockUniverse,
};

/// Internal cache entry with timestamp
struct CacheEntry<T> {
    data: T,
    cached_at: DateTime<Utc>,
}

pub struct AnalysisOrchestrator {
    pub polygon_client: PolygonClient,
    technical_analyzer: TechnicalAnalysisEngine,
    fundamental_analyzer: FundamentalAnalysisEngine,
    quant_analyzer: QuantAnalysisEngine,
    sentiment_analyzer: SentimentAnalysisEngine,
    /// Optional ML signal models client for dynamic weights
    signal_models_client: Option<SignalModelsClient>,
    /// Optional database pool for logging analysis features
    db_pool: Option<sqlx::AnyPool>,
    /// Cache news articles per symbol (5-min TTL)
    news_cache: DashMap<String, CacheEntry<Vec<NewsArticle>>>,
    /// Cache bars per (symbol, timeframe_key, days) (5-min TTL)
    bars_cache: DashMap<String, CacheEntry<Vec<Bar>>>,
    /// Secondary index for fast superset lookup: "AAPL:1:day" -> [30, 90, 365]
    bars_days_index: DashMap<String, Vec<i64>>,
    /// Cache ticker details per symbol (5-min TTL)
    ticker_details_cache: DashMap<String, CacheEntry<TickerDetails>>,
    /// Cache financials per symbol (5-min TTL)
    financials_cache: DashMap<String, CacheEntry<Vec<Financials>>>,
    /// Cache analyst consensus per symbol (5-min TTL)
    consensus_cache: DashMap<String, CacheEntry<AnalystConsensusData>>,
}

const CACHE_TTL_SECS: i64 = 300; // 5 minutes

impl AnalysisOrchestrator {
    pub fn new(polygon_api_key: String) -> Self {
        // Try to create signal models client from env
        let signal_models_url = std::env::var("ML_SIGNAL_MODELS_URL")
            .unwrap_or_else(|_| "http://localhost:8004".to_string());
        let signal_models_client = Some(SignalModelsClient::new(
            signal_models_url,
            std::time::Duration::from_secs(5),
        ));

        Self {
            polygon_client: PolygonClient::new(polygon_api_key),
            technical_analyzer: TechnicalAnalysisEngine::new(),
            fundamental_analyzer: FundamentalAnalysisEngine::new(),
            quant_analyzer: QuantAnalysisEngine::new(),
            sentiment_analyzer: SentimentAnalysisEngine::new(),
            signal_models_client,
            db_pool: None,
            news_cache: DashMap::new(),
            bars_cache: DashMap::new(),
            bars_days_index: DashMap::new(),
            ticker_details_cache: DashMap::new(),
            financials_cache: DashMap::new(),
            consensus_cache: DashMap::new(),
        }
    }

    /// Set the database pool for logging analysis features
    pub fn with_db_pool(mut self, pool: sqlx::AnyPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Public accessor for the technical analysis engine (used by point-in-time backtesting)
    pub fn technical_engine(&self) -> &TechnicalAnalysisEngine {
        &self.technical_analyzer
    }

    /// Public accessor for the quant analysis engine (used by point-in-time backtesting)
    pub fn quant_engine(&self) -> &QuantAnalysisEngine {
        &self.quant_analyzer
    }

    /// Enhanced market regime detection: combines trend direction (bull/bear) with volatility state.
    /// Returns composite regimes like "high_vol_bear", "low_vol_bull", "normal_bull", etc.
    fn detect_market_regime(&self, spy_bars: &[Bar]) -> String {
        if spy_bars.len() < 50 {
            return "unknown".to_string();
        }

        let returns: Vec<f64> = spy_bars
            .windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();

        if returns.len() < 50 {
            return "unknown".to_string();
        }

        // Full-period volatility
        let full_mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let full_var =
            returns.iter().map(|r| (r - full_mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let full_vol = full_var.sqrt();

        // Recent 10-day volatility
        let recent = &returns[returns.len() - 10..];
        let recent_mean = recent.iter().sum::<f64>() / recent.len() as f64;
        let recent_var = recent
            .iter()
            .map(|r| (r - recent_mean).powi(2))
            .sum::<f64>()
            / recent.len() as f64;
        let recent_vol = recent_var.sqrt();

        // Adaptive volatility regime detection using rolling vol ratios
        let mut vol_ratios: Vec<f64> = Vec::new();
        for i in 10..returns.len() {
            let window = &returns[i - 10..i];
            let w_mean = window.iter().sum::<f64>() / 10.0;
            let w_var = window.iter().map(|r| (r - w_mean).powi(2)).sum::<f64>() / 10.0;
            let w_vol = w_var.sqrt();
            if full_vol > 0.0 {
                vol_ratios.push(w_vol / full_vol);
            }
        }
        let current_ratio = if full_vol > 0.0 {
            recent_vol / full_vol
        } else {
            1.0
        };
        let vol_pct = adaptive::percentile_rank(current_ratio, &vol_ratios);
        let vol_regime = if vol_pct > 0.85 {
            "high_vol"
        } else if vol_pct < 0.15 {
            "low_vol"
        } else {
            "normal"
        };

        // Adaptive trend detection using momentum and SMA distance percentiles
        let closes: Vec<f64> = spy_bars.iter().map(|b| b.close).collect();
        let sma_50: f64 = closes[closes.len().saturating_sub(50)..]
            .iter()
            .sum::<f64>()
            / closes.len().min(50) as f64;
        let current_price = closes.last().copied().unwrap_or(0.0);

        // Rolling 20-day momentum distribution
        let mut momentums: Vec<f64> = Vec::new();
        for i in 20..closes.len() {
            let p0 = closes[i - 20];
            if p0 > 0.0 {
                momentums.push((closes[i] - p0) / p0);
            }
        }
        let momentum_20 = if closes.len() >= 20 {
            let p20 = closes[closes.len() - 20];
            if p20 > 0.0 {
                (current_price - p20) / p20
            } else {
                0.0
            }
        } else {
            0.0
        };
        let momentum_pct = adaptive::percentile_rank(momentum_20, &momentums);

        // Rolling SMA-50 distance distribution
        let sma_dist = if sma_50 > 0.0 {
            (current_price - sma_50) / sma_50
        } else {
            0.0
        };
        let mut sma_dists: Vec<f64> = Vec::new();
        for i in 50..closes.len() {
            let sma = closes[i.saturating_sub(50)..i].iter().sum::<f64>() / 50.min(i) as f64;
            if sma > 0.0 {
                sma_dists.push((closes[i] - sma) / sma);
            }
        }
        let sma_pct = adaptive::percentile_rank(sma_dist, &sma_dists);

        let trend = if sma_pct > 0.70 && momentum_pct > 0.65 {
            "bull"
        } else if sma_pct < 0.30 && momentum_pct < 0.35 {
            "bear"
        } else {
            "sideways"
        };

        format!("{}_{}", vol_regime, trend)
    }

    /// Get regime-conditional default engine weights.
    /// Returns (technical, fundamental, quant, sentiment) as percentages.
    fn regime_default_weights(&self, regime: &str) -> (i32, i32, i32, i32) {
        match regime {
            "high_vol_bear" => (15, 30, 35, 20), // Lean on risk/quant in volatile downtrends
            "high_vol_bull" => (25, 25, 30, 20), // Quant risk still important in volatile uptrends
            "high_vol_sideways" => (15, 30, 35, 20),
            "low_vol_bull" => (30, 30, 15, 25), // Technical momentum + sentiment in calm uptrends
            "low_vol_bear" => (20, 40, 20, 20), // Fundamental value focus in slow decline
            "low_vol_sideways" => (25, 35, 20, 20),
            "normal_bull" => (25, 35, 15, 25), // Balanced with slight fundamental tilt
            "normal_bear" => (20, 35, 25, 20), // More quant risk-awareness
            "normal_sideways" => (20, 40, 15, 25), // Standard balanced
            _ => (20, 40, 15, 25),             // Fallback
        }
    }

    /// Compute conviction tier based on engine alignment and confidence.
    fn compute_conviction(
        &self,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
    ) -> String {
        let mut bullish = 0;
        let mut bearish = 0;
        let mut total_confidence = 0.0;
        let mut count = 0;

        for result in [technical, fundamental, quantitative, sentiment]
            .into_iter()
            .flatten()
        {
            let score = result.signal.to_score();
            if score >= 20 {
                bullish += 1;
            } else if score <= -20 {
                bearish += 1;
            }
            total_confidence += result.confidence;
            count += 1;
        }

        let avg_confidence = if count > 0 {
            total_confidence / count as f64
        } else {
            0.0
        };
        let agreement = bullish.max(bearish);

        if agreement >= 3 && avg_confidence > 0.65 {
            "HIGH".to_string()
        } else if agreement >= 2 && avg_confidence > 0.5 {
            "MODERATE".to_string()
        } else {
            "LOW".to_string()
        }
    }

    /// Build time-horizon signal breakdown.
    fn build_time_horizon_signals(
        &self,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
    ) -> serde_json::Value {
        let mut horizons = serde_json::Map::new();

        // Short-term: technical + sentiment (days to weeks)
        let mut short_score = 0i32;
        let mut short_weight = 0i32;
        if let Some(tech) = technical {
            short_score += tech.signal.to_score() * 60;
            short_weight += 60;
        }
        if let Some(sent) = sentiment {
            short_score += sent.signal.to_score() * 40;
            short_weight += 40;
        }
        if short_weight > 0 {
            let short_signal =
                SignalStrength::from_score((short_score as f64 / short_weight as f64) as i32);
            horizons.insert(
                "short_term".to_string(),
                json!({
                    "signal": short_signal.to_label(),
                    "horizon": "days to weeks",
                    "drivers": ["technical", "sentiment"],
                }),
            );
        }

        // Medium-term: quant (weeks to months)
        if let Some(quant) = quantitative {
            horizons.insert(
                "medium_term".to_string(),
                json!({
                    "signal": quant.signal.to_label(),
                    "horizon": "weeks to months",
                    "drivers": ["quantitative"],
                }),
            );
        }

        // Long-term: fundamental (months to quarters)
        if let Some(fund) = fundamental {
            horizons.insert(
                "long_term".to_string(),
                json!({
                    "signal": fund.signal.to_label(),
                    "horizon": "months to quarters",
                    "drivers": ["fundamental"],
                }),
            );
        }

        serde_json::Value::Object(horizons)
    }

    /// Perform comprehensive analysis on a symbol
    pub async fn analyze(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        days_back: i64,
    ) -> Result<UnifiedAnalysis, AnalysisError> {
        tracing::info!(
            "Starting comprehensive analysis for {} (timeframe: {:?}, days: {})",
            symbol,
            timeframe,
            days_back
        );

        // Fire all API calls concurrently — Starter plan supports ~100 req/sec.
        // Cached responses (SPY/TLT bars, repeat symbols) return instantly.
        let (
            bars_result,
            financials_result,
            news_result,
            ticker_details,
            spy_bars_result,
            tlt_bars_result,
            snapshot_result,
            iwm_bars_result,
            iwd_bars_result,
            iwf_bars_result,
        ) = tokio::join!(
            self.get_bars(symbol, timeframe, days_back),
            self.get_financials(symbol),
            self.get_news(symbol, 50),
            self.get_ticker_details(symbol),
            self.get_bars("SPY", Timeframe::Day1, 365),
            self.get_bars("TLT", Timeframe::Day1, 90),
            self.polygon_client.get_snapshot(symbol),
            self.get_bars("IWM", Timeframe::Day1, 365),
            self.get_bars("IWD", Timeframe::Day1, 365),
            self.get_bars("IWF", Timeframe::Day1, 365),
        );

        // Use snapshot last trade price as primary current_price, fall back to last bar close
        let snapshot_price = snapshot_result
            .as_ref()
            .ok()
            .and_then(|snap| snap.last_trade.as_ref())
            .and_then(|lt| lt.p);

        let bar_price = bars_result
            .as_ref()
            .ok()
            .and_then(|bars| {
                tracing::info!("Bars count for {}: {}", symbol, bars.len());
                bars.last()
            })
            .map(|bar| bar.close);

        let current_price = snapshot_price.or(bar_price);
        if let Some(price) = current_price {
            tracing::info!(
                "Current price for {}: {} (source: {})",
                symbol,
                price,
                if snapshot_price.is_some() {
                    "snapshot"
                } else {
                    "bar close"
                }
            );
        } else {
            tracing::warn!(
                "No current price found for {} - bars_result error: {:?}",
                symbol,
                bars_result.as_ref().err()
            );
        }

        let spy_bars_ok = spy_bars_result.ok();
        let iwm_bars_ok = iwm_bars_result.ok();
        let iwd_bars_ok = iwd_bars_result.ok();
        let iwf_bars_ok = iwf_bars_result.ok();

        // Extract shares outstanding from ticker details for DCF model
        let shares_outstanding = ticker_details.as_ref().ok().and_then(|d| {
            d.weighted_shares_outstanding
                .or(d.share_class_shares_outstanding)
        });

        // Derive dynamic risk-free rate from TLT price
        let dynamic_risk_free_rate = tlt_bars_result.ok().and_then(|tlt_bars| {
            if tlt_bars.len() >= 2 {
                let first = tlt_bars.first().unwrap();
                let last = tlt_bars.last().unwrap();
                let tlt_return = (last.close - first.close) / first.close;
                // TLT inversely tracks yields: if TLT fell, rates rose
                let rate = (0.045 - tlt_return * 0.10).clamp(0.01, 0.08);
                tracing::info!(
                    "Dynamic risk-free rate from TLT: {:.3} (TLT return: {:.3})",
                    rate,
                    tlt_return
                );
                Some(rate)
            } else {
                None
            }
        });

        // Run all independent analysis engines concurrently.
        // Technical & quant are CPU-bound but fast (sub-ms on a few hundred bars).
        // Sentiment & consensus are async network calls to ML/Polygon services.
        // Running them in parallel overlaps the network latency.
        let (technical_result, quant_result, consensus_data, sentiment_result) = tokio::join!(
            async {
                if let Ok(bars) = &bars_result {
                    if bars.len() >= 50 {
                        tracing::info!(
                            "Running enhanced technical analysis with {} bars",
                            bars.len()
                        );
                        match self.technical_analyzer.analyze_enhanced(
                            symbol,
                            bars,
                            spy_bars_ok.as_deref(),
                        ) {
                            Ok(result) => return Some(result),
                            Err(e) => tracing::warn!("Technical analysis failed: {:?}", e),
                        }
                    }
                }
                None
            },
            async {
                if let Ok(bars) = &bars_result {
                    if bars.len() >= 30 {
                        tracing::info!("Running enhanced quantitative analysis");
                        match self.quant_analyzer.analyze_with_factors(
                            symbol,
                            bars,
                            spy_bars_ok.as_deref(),
                            iwm_bars_ok.as_deref(),
                            iwd_bars_ok.as_deref(),
                            iwf_bars_ok.as_deref(),
                            dynamic_risk_free_rate,
                        ) {
                            Ok(result) => return Some(result),
                            Err(e) => tracing::warn!("Quant analysis failed: {:?}", e),
                        }
                    }
                }
                None
            },
            self.get_analyst_consensus(symbol),
            async {
                if let Ok(news) = &news_result {
                    tracing::info!("Running sentiment analysis with {} articles", news.len());
                    if let Ok(result) = self.sentiment_analyzer.analyze(symbol, news).await {
                        return Some(result);
                    }
                }
                None
            },
        );

        // Fundamental analysis depends on consensus data, so it runs after the parallel phase
        let mut fundamental_result = None;
        if let Ok(financials_vec) = &financials_result {
            if !financials_vec.is_empty() {
                tracing::info!("Running enhanced fundamental analysis with consensus data");
                let sic_desc = ticker_details
                    .as_ref()
                    .ok()
                    .and_then(|d| d.sic_description.as_deref());
                match self.fundamental_analyzer.analyze_with_consensus(
                    symbol,
                    financials_vec,
                    current_price,
                    shares_outstanding,
                    &consensus_data,
                    dynamic_risk_free_rate,
                    sic_desc,
                ) {
                    Ok(result) => fundamental_result = Some(result),
                    Err(e) => tracing::warn!("Fundamental analysis failed: {:?}", e),
                }
            }
        }

        // Detect market regime from SPY bars (needed for regime-conditional weights)
        let market_regime = spy_bars_ok
            .as_deref()
            .map(|spy_bars| self.detect_market_regime(spy_bars));

        // Combine results (now async — may fetch dynamic weights from ML service)
        let mut overall = self
            .combine_results(
                symbol,
                &technical_result,
                &fundamental_result,
                &quant_result,
                &sentiment_result,
                market_regime.as_deref(),
            )
            .await;
        overall.current_price = current_price;
        overall.name = ticker_details.ok().map(|d| d.name);
        overall.market_regime = market_regime;

        // Compute supplementary signals from options, insiders, dividends, snapshot
        let (supplementary, confidence_adj) = self
            .compute_supplementary_signals(symbol, current_price, bars_result.as_ref().ok())
            .await;
        overall.supplementary_signals = Some(supplementary);
        overall.overall_confidence =
            (overall.overall_confidence + confidence_adj).clamp(0.05, 0.98);

        // Log analysis features for future model training (fire-and-forget)
        self.log_analysis_features(
            symbol,
            &technical_result,
            &fundamental_result,
            &quant_result,
            &sentiment_result,
            &overall.overall_signal,
            overall.overall_confidence,
            overall.market_regime.as_deref(),
            overall.conviction_tier.as_deref(),
        );

        Ok(overall)
    }

    /// Combine individual analysis results into unified analysis.
    /// Uses: ML-predicted weights > regime-conditional weights > hardcoded defaults.
    async fn combine_results(
        &self,
        symbol: &str,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
        market_regime: Option<&str>,
    ) -> UnifiedAnalysis {
        // Try to get dynamic weights from signal models service
        let dynamic_weights = self
            .try_get_dynamic_weights(technical, fundamental, quantitative, sentiment)
            .await;

        // Priority: ML weights > regime-conditional > hardcoded
        let (w_tech, w_fund, w_quant, w_sent) = match &dynamic_weights {
            Some(w) => {
                let wt = (w.get("technical").copied().unwrap_or(0.20) * 100.0) as i32;
                let wf = (w.get("fundamental").copied().unwrap_or(0.40) * 100.0) as i32;
                let wq = (w.get("quantitative").copied().unwrap_or(0.15) * 100.0) as i32;
                let ws = (w.get("sentiment").copied().unwrap_or(0.25) * 100.0) as i32;
                (wt, wf, wq, ws)
            }
            None => self.regime_default_weights(market_regime.unwrap_or("unknown")),
        };

        let mut total_score = 0;
        let mut total_weight = 0;
        let mut combined_confidence = 0.0;
        let mut count = 0;

        if let Some(tech) = technical {
            total_score += tech.signal.to_score() * w_tech;
            total_weight += w_tech;
            combined_confidence += tech.confidence * (w_tech as f64 / 100.0);
            count += 1;
        }

        if let Some(fund) = fundamental {
            total_score += fund.signal.to_score() * w_fund;
            total_weight += w_fund;
            combined_confidence += fund.confidence * (w_fund as f64 / 100.0);
            count += 1;
        }

        if let Some(quant) = quantitative {
            total_score += quant.signal.to_score() * w_quant;
            total_weight += w_quant;
            combined_confidence += quant.confidence * (w_quant as f64 / 100.0);
            count += 1;
        }

        if let Some(sent) = sentiment {
            total_score += sent.signal.to_score() * w_sent;
            total_weight += w_sent;
            combined_confidence += sent.confidence * (w_sent as f64 / 100.0);
            count += 1;
        }

        let overall_signal = if total_weight > 0 {
            SignalStrength::from_score((total_score as f64 / total_weight as f64) as i32)
        } else {
            SignalStrength::Neutral
        };

        // Penalize confidence when engines conflict (e.g., one says Buy, another says Sell)
        let mut scores: Vec<i32> = Vec::new();
        if let Some(tech) = technical {
            scores.push(tech.signal.to_score());
        }
        if let Some(fund) = fundamental {
            scores.push(fund.signal.to_score());
        }
        if let Some(quant) = quantitative {
            scores.push(quant.signal.to_score());
        }
        if let Some(sent) = sentiment {
            scores.push(sent.signal.to_score());
        }

        let conflict_penalty = if scores.len() >= 2 {
            let has_bullish = scores.iter().any(|&s| s >= 30);
            let has_bearish = scores.iter().any(|&s| s <= -30);
            if has_bullish && has_bearish {
                let max_score = scores.iter().max().copied().unwrap_or(0);
                let min_score = scores.iter().min().copied().unwrap_or(0);
                let spread = (max_score - min_score) as f64 / 200.0;
                spread * 0.30
            } else {
                0.0
            }
        } else {
            0.0
        };

        let overall_confidence = if count > 0 {
            (combined_confidence - conflict_penalty).max(0.05)
        } else {
            0.0
        };

        // Compute conviction tier and time horizon signals
        let conviction_tier =
            self.compute_conviction(technical, fundamental, quantitative, sentiment);
        let time_horizon_signals =
            self.build_time_horizon_signals(technical, fundamental, quantitative, sentiment);

        // Enhanced recommendation with conviction
        let recommendation = format!(
            "{} [{}]",
            self.generate_recommendation(&overall_signal, overall_confidence),
            conviction_tier,
        );

        UnifiedAnalysis {
            symbol: symbol.to_string(),
            name: None,
            timestamp: Utc::now(),
            current_price: None,
            technical: technical.clone(),
            fundamental: fundamental.clone(),
            quantitative: quantitative.clone(),
            sentiment: sentiment.clone(),
            overall_signal,
            overall_confidence,
            recommendation,
            market_regime: market_regime.map(|s| s.to_string()),
            conviction_tier: Some(conviction_tier),
            time_horizon_signals: Some(time_horizon_signals),
            supplementary_signals: None, // Set by caller after fetching options/insiders/dividends
        }
    }

    /// Try to get dynamic weights from the signal models service.
    /// Returns None on any error (graceful fallback to hardcoded weights).
    async fn try_get_dynamic_weights(
        &self,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
    ) -> Option<HashMap<String, f64>> {
        let client = self.signal_models_client.as_ref()?;

        let mut features = HashMap::new();
        // Build minimal feature set for weight prediction
        if let Some(tech) = technical {
            features.insert("technical_score".to_string(), tech.signal.to_score() as f64);
            features.insert("technical_confidence".to_string(), tech.confidence);
        }
        if let Some(fund) = fundamental {
            features.insert(
                "fundamental_score".to_string(),
                fund.signal.to_score() as f64,
            );
            features.insert("fundamental_confidence".to_string(), fund.confidence);
        }
        if let Some(quant) = quantitative {
            features.insert("quant_score".to_string(), quant.signal.to_score() as f64);
            features.insert("quant_confidence".to_string(), quant.confidence);
        }
        if let Some(sent) = sentiment {
            features.insert("sentiment_score".to_string(), sent.signal.to_score() as f64);
            features.insert("sentiment_confidence".to_string(), sent.confidence);
        }

        match client.get_optimal_weights(&features).await {
            Ok(engine_weights) => {
                tracing::info!(
                    "Using dynamic weights from signal models: {:?}",
                    engine_weights.weights
                );
                Some(engine_weights.weights)
            }
            Err(e) => {
                tracing::debug!("Signal models unavailable, using default weights: {}", e);
                None
            }
        }
    }

    /// Extract feature vector from analysis results and log to DB (fire-and-forget).
    #[allow(clippy::too_many_arguments)]
    fn log_analysis_features(
        &self,
        symbol: &str,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
        overall_signal: &SignalStrength,
        overall_confidence: f64,
        market_regime: Option<&str>,
        conviction_tier: Option<&str>,
    ) {
        let pool = match &self.db_pool {
            Some(p) => p.clone(),
            None => return,
        };

        let mut features = HashMap::new();

        // Engine signals & confidences
        if let Some(tech) = technical {
            features.insert("technical_score".to_string(), tech.signal.to_score() as f64);
            features.insert("technical_confidence".to_string(), tech.confidence);
            // Extract key metrics from JSON
            if let Some(metrics) = tech.metrics.as_object() {
                for key in &["rsi", "bb_percent_b", "adx", "sma_20", "sma_50"] {
                    if let Some(v) = metrics.get(*key).and_then(|v| v.as_f64()) {
                        features.insert(key.to_string(), v);
                    }
                }
            }
        }
        if let Some(fund) = fundamental {
            features.insert(
                "fundamental_score".to_string(),
                fund.signal.to_score() as f64,
            );
            features.insert("fundamental_confidence".to_string(), fund.confidence);
            if let Some(metrics) = fund.metrics.as_object() {
                for key in &["pe_ratio", "debt_to_equity", "revenue_growth", "roic"] {
                    if let Some(v) = metrics.get(*key).and_then(|v| v.as_f64()) {
                        features.insert(key.to_string(), v);
                    }
                }
            }
        }
        if let Some(quant) = quantitative {
            features.insert("quant_score".to_string(), quant.signal.to_score() as f64);
            features.insert("quant_confidence".to_string(), quant.confidence);
            if let Some(metrics) = quant.metrics.as_object() {
                for key in &["sharpe_ratio", "volatility", "max_drawdown", "beta"] {
                    if let Some(v) = metrics.get(*key).and_then(|v| v.as_f64()) {
                        features.insert(key.to_string(), v);
                    }
                }
            }
        }
        if let Some(sent) = sentiment {
            features.insert("sentiment_score".to_string(), sent.signal.to_score() as f64);
            features.insert("sentiment_confidence".to_string(), sent.confidence);
            if let Some(metrics) = sent.metrics.as_object() {
                for key in &["normalized_score", "article_count", "direct_mention_ratio"] {
                    if let Some(v) = metrics.get(*key).and_then(|v| v.as_f64()) {
                        features.insert(key.to_string(), v);
                    }
                }
            }
        }

        // Market context
        let regime = market_regime.unwrap_or("normal");
        let regime_encoded = match regime {
            "low_volatility" => -1.0,
            "high_volatility" => 1.0,
            _ => 0.0,
        };
        features.insert("market_regime_encoded".to_string(), regime_encoded);

        let signal_str = format!("{:?}", overall_signal);
        let symbol_owned = symbol.to_string();
        let analysis_date = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        // Build JSON with numeric features + string metadata for analytics
        let mut features_value = serde_json::to_value(&features).unwrap_or_default();
        if let Some(obj) = features_value.as_object_mut() {
            obj.insert("market_regime".to_string(), serde_json::json!(regime));
            obj.insert(
                "conviction_tier".to_string(),
                serde_json::json!(conviction_tier.unwrap_or("UNKNOWN")),
            );
        }
        let features_json = match serde_json::to_string(&features_value) {
            Ok(j) => j,
            Err(_) => return,
        };

        // Fire-and-forget: spawn a task to insert into DB
        tokio::spawn(async move {
            let result = sqlx::query(
                "INSERT INTO analysis_features (symbol, analysis_date, features_json, overall_signal, overall_confidence) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&symbol_owned)
            .bind(&analysis_date)
            .bind(&features_json)
            .bind(&signal_str)
            .bind(overall_confidence)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::debug!("Failed to log analysis features: {}", e);
            }
        });
    }

    /// Compute supplementary signals from options, insiders, dividends, and snapshot.
    /// Returns (signals_json, score_adjustment) where score_adjustment modifies overall confidence.
    async fn compute_supplementary_signals(
        &self,
        symbol: &str,
        current_price: Option<f64>,
        bars: Option<&Vec<Bar>>,
    ) -> (serde_json::Value, f64) {
        let mut signals = serde_json::Map::new();
        let mut score_adj = 0.0_f64;

        // Fetch supplementary data concurrently (graceful errors)
        let (options_result, insiders_result, dividends_result) = tokio::join!(
            self.polygon_client.get_options_snapshot(symbol),
            self.polygon_client.get_insider_transactions(symbol, 50),
            self.polygon_client.get_dividends(symbol, 20),
        );

        // --- Options-Implied Intelligence ---
        if let Ok(options) = &options_result {
            if !options.is_empty() {
                let mut call_oi = 0i64;
                let mut put_oi = 0i64;
                let mut call_iv_sum = 0.0_f64;
                let mut put_iv_sum = 0.0_f64;
                let mut call_iv_count = 0u32;
                let mut put_iv_count = 0u32;
                let mut ivs: Vec<f64> = Vec::new();

                for opt in options {
                    let contract_type = opt
                        .details
                        .as_ref()
                        .and_then(|d| d.contract_type.as_deref())
                        .unwrap_or("");
                    let oi = opt.open_interest.unwrap_or(0);
                    let iv = opt.implied_volatility.unwrap_or(0.0);

                    if iv > 0.0 {
                        ivs.push(iv);
                    }

                    if contract_type.eq_ignore_ascii_case("call") {
                        call_oi += oi;
                        if iv > 0.0 {
                            call_iv_sum += iv;
                            call_iv_count += 1;
                        }
                    } else if contract_type.eq_ignore_ascii_case("put") {
                        put_oi += oi;
                        if iv > 0.0 {
                            put_iv_sum += iv;
                            put_iv_count += 1;
                        }
                    }
                }

                // Adaptive Put/Call ratio thresholds using per-strike distribution
                let pc_ratio = if call_oi > 0 {
                    put_oi as f64 / call_oi as f64
                } else {
                    1.0
                };
                let mut per_strike_pc_ratios: Vec<f64> = Vec::new();
                let mut strike_call_oi: std::collections::HashMap<i64, i64> =
                    std::collections::HashMap::new();
                let mut strike_put_oi: std::collections::HashMap<i64, i64> =
                    std::collections::HashMap::new();
                for opt in options {
                    if let Some(strike) = opt.details.as_ref().and_then(|d| d.strike_price) {
                        let key = (strike * 100.0) as i64;
                        let oi = opt.open_interest.unwrap_or(0);
                        let contract_type = opt
                            .details
                            .as_ref()
                            .and_then(|d| d.contract_type.as_deref())
                            .unwrap_or("");
                        if contract_type.eq_ignore_ascii_case("call") {
                            *strike_call_oi.entry(key).or_insert(0) += oi;
                        } else if contract_type.eq_ignore_ascii_case("put") {
                            *strike_put_oi.entry(key).or_insert(0) += oi;
                        }
                    }
                }
                for (strike, p_oi) in &strike_put_oi {
                    if let Some(&c_oi) = strike_call_oi.get(strike) {
                        if c_oi > 0 {
                            per_strike_pc_ratios.push(*p_oi as f64 / c_oi as f64);
                        }
                    }
                }

                let (pc_signal, pc_adj) = if !per_strike_pc_ratios.is_empty() {
                    let pc_pct = adaptive::percentile_rank(pc_ratio, &per_strike_pc_ratios);
                    if pc_pct < 0.15 {
                        ("bullish", 0.03)
                    } else if pc_pct > 0.85 {
                        ("bearish", -0.03)
                    } else {
                        ("neutral", 0.0)
                    }
                } else {
                    // Fallback to z-score with baseline mean=1.0, std=0.3
                    let baseline_data = vec![0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];
                    let z = adaptive::z_score_of(pc_ratio, &baseline_data);
                    if z < -1.5 {
                        ("bullish", 0.03)
                    } else if z > 1.5 {
                        ("bearish", -0.03)
                    } else {
                        ("neutral", 0.0)
                    }
                };
                score_adj += pc_adj;

                // Adaptive IV Skew using z-score relative to observed IV distribution
                let avg_call_iv = if call_iv_count > 0 {
                    call_iv_sum / call_iv_count as f64
                } else {
                    0.0
                };
                let avg_put_iv = if put_iv_count > 0 {
                    put_iv_sum / put_iv_count as f64
                } else {
                    0.0
                };
                let iv_skew = if avg_call_iv > 0.0 {
                    avg_put_iv / avg_call_iv
                } else {
                    1.0
                };
                let skew_z = adaptive::z_score_of(iv_skew, &ivs);
                let skew_signal = if skew_z > 1.5 {
                    "heavy_put_demand"
                } else if skew_z < -1.5 {
                    "heavy_call_demand"
                } else {
                    "balanced"
                };

                // IV Percentile (where is current median IV vs all observed IVs)
                let iv_percentile = if !ivs.is_empty() {
                    ivs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let median_iv = ivs[ivs.len() / 2];
                    // Rough percentile using current median
                    let below = ivs.iter().filter(|&&v| v <= median_iv).count();
                    (below as f64 / ivs.len() as f64) * 100.0
                } else {
                    50.0
                };

                if iv_percentile > 80.0 {
                    score_adj -= 0.02;
                }
                // High IV = mean-reversion pressure
                else if iv_percentile < 20.0 {
                    score_adj += 0.02;
                } // Low IV = expansion likely

                // Max Pain (strike with most total OI)
                let mut strike_oi: std::collections::HashMap<i64, i64> =
                    std::collections::HashMap::new();
                for opt in options {
                    if let Some(strike) = opt.details.as_ref().and_then(|d| d.strike_price) {
                        let key = (strike * 100.0) as i64; // penny resolution
                        *strike_oi.entry(key).or_insert(0) += opt.open_interest.unwrap_or(0);
                    }
                }
                let max_pain = strike_oi
                    .into_iter()
                    .max_by_key(|(_, oi)| *oi)
                    .map(|(k, _)| k as f64 / 100.0);
                let max_pain_convergence = if let (Some(mp), Some(p)) = (max_pain, current_price) {
                    if p > 0.0 {
                        ((mp - p) / p * 100.0).abs()
                    } else {
                        100.0
                    }
                } else {
                    100.0
                };

                signals.insert(
                    "options".to_string(),
                    json!({
                        "put_call_ratio": pc_ratio,
                        "put_call_signal": pc_signal,
                        "iv_skew": iv_skew,
                        "iv_skew_signal": skew_signal,
                        "iv_percentile": iv_percentile,
                        "max_pain": max_pain,
                        "max_pain_distance_pct": max_pain_convergence,
                        "call_open_interest": call_oi,
                        "put_open_interest": put_oi,
                        "total_contracts": options.len(),
                    }),
                );
            }
        }

        // --- Insider Transaction Signals (adaptive thresholds) ---
        if let Ok(insiders) = &insiders_result {
            if !insiders.is_empty() {
                let mut buy_value = 0.0_f64;
                let mut sell_value = 0.0_f64;
                let mut buy_count = 0u32;
                let mut sell_count = 0u32;
                let mut executive_buys = 0u32;

                for txn in insiders {
                    let is_buy = txn
                        .transaction_type
                        .as_deref()
                        .map(|t| {
                            let tl = t.to_lowercase();
                            tl.contains("buy")
                                || tl.contains("purchase")
                                || tl.contains("acquisition")
                        })
                        .unwrap_or(false);
                    let is_sell = txn
                        .transaction_type
                        .as_deref()
                        .map(|t| {
                            let tl = t.to_lowercase();
                            tl.contains("sell") || tl.contains("sale") || tl.contains("disposition")
                        })
                        .unwrap_or(false);

                    let value = txn.total_value.unwrap_or(0.0).abs();
                    let is_executive = txn
                        .title
                        .as_deref()
                        .map(|t| {
                            let tl = t.to_lowercase();
                            tl.contains("ceo")
                                || tl.contains("cfo")
                                || tl.contains("coo")
                                || tl.contains("president")
                                || tl.contains("chief")
                        })
                        .unwrap_or(false);

                    if is_buy {
                        buy_value += value;
                        buy_count += 1;
                        if is_executive {
                            executive_buys += 1;
                        }
                    } else if is_sell {
                        sell_value += value;
                        sell_count += 1;
                    }
                }

                // Adaptive insider thresholds based on typical daily traded value
                let typical_daily_value = bars
                    .map(|b| {
                        let recent = &b[b.len().saturating_sub(20)..];
                        let avg_vol =
                            recent.iter().map(|bar| bar.volume).sum::<f64>() / recent.len() as f64;
                        let avg_price =
                            recent.iter().map(|bar| bar.close).sum::<f64>() / recent.len() as f64;
                        avg_vol * avg_price
                    })
                    .unwrap_or(10_000_000.0);

                let significant_buy_threshold = typical_daily_value * 0.001; // 0.1% of daily value
                let significant_sell_threshold = typical_daily_value * 0.005; // 0.5% of daily value

                let net_value = buy_value - sell_value;
                let insider_signal = if net_value > significant_buy_threshold
                    && buy_count > sell_count
                {
                    score_adj += 0.04;
                    "bullish"
                } else if net_value < -significant_sell_threshold && sell_count > buy_count * 2 {
                    score_adj -= 0.03;
                    "bearish"
                } else {
                    "neutral"
                };

                if executive_buys >= 2 {
                    score_adj += 0.03; // Multiple C-suite buys is very bullish
                }

                signals.insert(
                    "insiders".to_string(),
                    json!({
                        "buy_count": buy_count,
                        "sell_count": sell_count,
                        "buy_value": buy_value,
                        "sell_value": sell_value,
                        "net_value": net_value,
                        "executive_buys": executive_buys,
                        "signal": insider_signal,
                    }),
                );
            }
        }

        // --- Dividend Health Signals (adaptive thresholds) ---
        if let Ok(dividends) = &dividends_result {
            if dividends.len() >= 2 {
                let amounts: Vec<f64> = dividends.iter().filter_map(|d| d.cash_amount).collect();

                if amounts.len() >= 2 {
                    let latest = amounts[0];
                    let prev = amounts[1];
                    let div_change = if prev > 0.0 {
                        (latest - prev) / prev * 100.0
                    } else {
                        0.0
                    };

                    // Compute all consecutive dividend changes for z-score
                    let mut div_changes: Vec<f64> = Vec::new();
                    for i in 1..amounts.len() {
                        if amounts[i] > 0.0 {
                            div_changes.push((amounts[i - 1] - amounts[i]) / amounts[i] * 100.0);
                        }
                    }

                    // Adaptive dividend change detection using z-score
                    let div_signal = if latest <= 0.0 && prev > 0.0 {
                        score_adj -= 0.05;
                        "cut_or_suspended"
                    } else if !div_changes.is_empty() {
                        let z = adaptive::z_score_of(div_change, &div_changes);
                        if z < -1.5 {
                            score_adj -= 0.03;
                            "significant_cut"
                        } else if z > 1.5 {
                            score_adj += 0.02;
                            "significant_increase"
                        } else {
                            "stable"
                        }
                    } else if div_change < -10.0 {
                        score_adj -= 0.03;
                        "significant_cut"
                    } else if div_change > 10.0 {
                        score_adj += 0.02;
                        "significant_increase"
                    } else {
                        "stable"
                    };

                    // Annualized yield (if we have price)
                    let annual_div = if let Some(freq) = dividends[0].frequency {
                        latest * freq as f64
                    } else {
                        latest * 4.0 // assume quarterly
                    };
                    let div_yield = current_price
                        .filter(|&p| p > 0.0)
                        .map(|p| annual_div / p * 100.0);

                    // Special dividend detection
                    let has_special = dividends.iter().any(|d| {
                        d.dividend_type
                            .as_deref()
                            .map(|t| t.to_lowercase().contains("special"))
                            .unwrap_or(false)
                    });
                    if has_special {
                        score_adj += 0.02;
                    }

                    signals.insert(
                        "dividends".to_string(),
                        json!({
                            "latest_amount": latest,
                            "change_pct": div_change,
                            "signal": div_signal,
                            "annual_yield_pct": div_yield,
                            "has_special_dividend": has_special,
                            "payment_count": amounts.len(),
                        }),
                    );
                }
            }
        }

        // --- Snapshot / Intraday Gap Analysis (adaptive thresholds) ---
        if let Ok(snapshot) = self.polygon_client.get_snapshot(symbol).await {
            if let (Some(day), Some(prev)) = (&snapshot.day, &snapshot.prev_day) {
                let today_open = day.o.unwrap_or(0.0);
                let prev_close = prev.c.unwrap_or(0.0);
                if prev_close > 0.0 && today_open > 0.0 {
                    let gap_pct = (today_open - prev_close) / prev_close * 100.0;

                    // Compute historical daily gaps from bars
                    let mut historical_gaps: Vec<f64> = Vec::new();
                    if let Some(bars) = bars {
                        for i in 1..bars.len() {
                            let prev_bar_close = bars[i - 1].close;
                            let curr_bar_open = bars[i].open;
                            if prev_bar_close > 0.0 && curr_bar_open > 0.0 {
                                historical_gaps.push(
                                    (curr_bar_open - prev_bar_close) / prev_bar_close * 100.0,
                                );
                            }
                        }
                    }

                    let gap_signal = if !historical_gaps.is_empty() {
                        let gap_pct_rank = adaptive::percentile_rank(gap_pct, &historical_gaps);
                        if gap_pct_rank > 0.90 {
                            "gap_up"
                        } else if gap_pct_rank < 0.10 {
                            "gap_down"
                        } else {
                            "flat"
                        }
                    } else {
                        // Fallback to fixed thresholds
                        if gap_pct > 2.0 {
                            "gap_up"
                        } else if gap_pct < -2.0 {
                            "gap_down"
                        } else {
                            "flat"
                        }
                    };

                    // Large gaps often fill — a gap up with weak follow-through is bearish
                    if let Some(today_close) = day.c {
                        if gap_signal == "gap_up" && today_close < today_open {
                            score_adj -= 0.02; // Gap up fading
                        } else if gap_signal == "gap_down" && today_close > today_open {
                            score_adj += 0.02; // Gap down reversing
                        }
                    }

                    let change_pct = snapshot.todays_change_perc.unwrap_or(0.0);

                    signals.insert(
                        "intraday".to_string(),
                        json!({
                            "gap_pct": gap_pct,
                            "gap_signal": gap_signal,
                            "change_pct": change_pct,
                            "today_open": today_open,
                            "prev_close": prev_close,
                        }),
                    );
                }
            }
        }

        // --- Smart Money Composite (adaptive thresholds) ---
        // Combines insider buys + options positioning + volume accumulation
        let mut smart_money_score = 0.0_f64;
        if let Some(insider_sig) = signals.get("insiders") {
            if insider_sig.get("signal").and_then(|s| s.as_str()) == Some("bullish") {
                smart_money_score += 1.0;
            }
            if insider_sig
                .get("executive_buys")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                >= 2
            {
                smart_money_score += 1.0;
            }
        }
        if let Some(opt_sig) = signals.get("options") {
            if opt_sig.get("put_call_signal").and_then(|s| s.as_str()) == Some("bullish") {
                smart_money_score += 1.0;
            }
        }
        // Check for quiet accumulation using adaptive volume decline threshold
        if let Some(bars) = bars {
            if bars.len() >= 20 {
                let recent = &bars[bars.len() - 10..];
                let prior = &bars[bars.len() - 20..bars.len() - 10];
                let recent_avg_vol: f64 = recent.iter().map(|b| b.volume).sum::<f64>() / 10.0;
                let prior_avg_vol: f64 = prior.iter().map(|b| b.volume).sum::<f64>() / 10.0;

                // Compute historical volume ratios for adaptive threshold
                let mut vol_ratios: Vec<f64> = Vec::new();
                for i in 20..bars.len() {
                    let recent_window = &bars[i - 10..i];
                    let prior_window = &bars[i - 20..i - 10];
                    let recent_vol: f64 =
                        recent_window.iter().map(|b| b.volume).sum::<f64>() / 10.0;
                    let prior_vol: f64 = prior_window.iter().map(|b| b.volume).sum::<f64>() / 10.0;
                    if prior_vol > 0.0 {
                        vol_ratios.push(recent_vol / prior_vol);
                    }
                }

                let current_vol_ratio = if prior_avg_vol > 0.0 {
                    recent_avg_vol / prior_avg_vol
                } else {
                    1.0
                };
                let vol_ratio_pct = adaptive::percentile_rank(current_vol_ratio, &vol_ratios);

                if vol_ratio_pct < 0.20 {
                    smart_money_score += 0.5; // Declining volume in bottom 20% = quiet accumulation
                }
            }
        }

        // Use z-score for smart money signal thresholds
        let smart_money_z = if smart_money_score != 0.0 {
            // Baseline distribution: typical scores range 0-3
            let baseline_scores = vec![-1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5];
            adaptive::z_score_of(smart_money_score, &baseline_scores)
        } else {
            0.0
        };

        let smart_money_signal = if smart_money_z > 2.0 {
            "strong_accumulation"
        } else if smart_money_z > 1.0 {
            "accumulation"
        } else if smart_money_z < -1.0 {
            "distribution"
        } else {
            "neutral"
        };

        // Adaptive confidence adjustment based on z-score magnitude
        let smart_money_adj = 0.02 * smart_money_z.abs().min(3.0) * smart_money_z.signum();
        score_adj += smart_money_adj;

        signals.insert(
            "smart_money".to_string(),
            json!({
                "score": smart_money_score,
                "signal": smart_money_signal,
            }),
        );

        // --- Earnings Transcript NLP ---
        let earnings_nlp_url = std::env::var("ML_EARNINGS_NLP_URL")
            .unwrap_or_else(|_| "http://localhost:8005".to_string());
        let earnings_client = ml_client::EarningsNlpClient::new(earnings_nlp_url);
        match earnings_client.analyze_earnings(symbol).await {
            Ok(nlp) if nlp.confidence > 0.0 && nlp.data_source != "none" => {
                // Tone-based adjustment
                match nlp.overall_tone.as_str() {
                    "positive" => score_adj += 0.03 * nlp.confidence,
                    "negative" => score_adj -= 0.03 * nlp.confidence,
                    _ => {}
                }
                // Guidance-based adjustment
                match nlp.guidance_sentiment.as_str() {
                    "raised" => score_adj += 0.02,
                    "lowered" => score_adj -= 0.02,
                    _ => {}
                }
                signals.insert(
                    "earnings_nlp".to_string(),
                    json!({
                        "overall_tone": nlp.overall_tone,
                        "tone_score": nlp.tone_score,
                        "confidence": nlp.confidence,
                        "guidance_sentiment": nlp.guidance_sentiment,
                        "guidance_keywords": nlp.guidance_keywords,
                        "forward_looking_count": nlp.forward_looking_count,
                        "risk_mentions": nlp.risk_mentions,
                        "key_topics": nlp.key_topics,
                        "data_source": nlp.data_source,
                    }),
                );
                tracing::info!(
                    "Earnings NLP for {}: tone={}, guidance={}",
                    symbol,
                    nlp.overall_tone,
                    nlp.guidance_sentiment
                );
            }
            Ok(_) => {
                tracing::debug!("Earnings NLP for {}: no data available", symbol);
            }
            Err(e) => {
                tracing::debug!("Earnings NLP unavailable for {}: {}", symbol, e);
            }
        }

        // --- Sector Rotation (adaptive thresholds) ---
        // Compare stock's recent performance vs sector ETF and SPY
        if let Some(bars) = bars {
            if bars.len() >= 20 {
                let stock_return_20d = {
                    let p0 = bars[bars.len() - 20].close;
                    let p1 = bars.last().unwrap().close;
                    if p0 > 0.0 {
                        (p1 - p0) / p0 * 100.0
                    } else {
                        0.0
                    }
                };

                // Compare vs SPY with adaptive thresholds
                if let Ok(spy_bars) = self.get_bars("SPY", Timeframe::Day1, 30).await {
                    if spy_bars.len() >= 20 {
                        let spy_return_20d = {
                            let p0 = spy_bars[spy_bars.len() - 20].close;
                            let p1 = spy_bars.last().unwrap().close;
                            if p0 > 0.0 {
                                (p1 - p0) / p0 * 100.0
                            } else {
                                0.0
                            }
                        };
                        let relative_perf = stock_return_20d - spy_return_20d;

                        // Compute historical relative performance distribution
                        let mut historical_rel_perf: Vec<f64> = Vec::new();
                        for i in 20..bars.len() {
                            let stock_ret = {
                                let p0 = bars[i - 20].close;
                                let p1 = bars[i].close;
                                if p0 > 0.0 {
                                    (p1 - p0) / p0 * 100.0
                                } else {
                                    0.0
                                }
                            };
                            if i < spy_bars.len() {
                                let spy_ret = {
                                    let p0 = spy_bars[i - 20].close;
                                    let p1 = spy_bars[i].close;
                                    if p0 > 0.0 {
                                        (p1 - p0) / p0 * 100.0
                                    } else {
                                        0.0
                                    }
                                };
                                historical_rel_perf.push(stock_ret - spy_ret);
                            }
                        }

                        let rotation_signal = if !historical_rel_perf.is_empty() {
                            let rel_perf_pct =
                                adaptive::percentile_rank(relative_perf, &historical_rel_perf);
                            if rel_perf_pct > 0.80 {
                                "outperforming"
                            } else if rel_perf_pct < 0.20 {
                                "underperforming"
                            } else {
                                "inline"
                            }
                        } else {
                            // Fallback to fixed thresholds
                            if relative_perf > 5.0 {
                                "outperforming"
                            } else if relative_perf < -5.0 {
                                "underperforming"
                            } else {
                                "inline"
                            }
                        };

                        signals.insert(
                            "sector_rotation".to_string(),
                            json!({
                                "stock_return_20d": stock_return_20d,
                                "spy_return_20d": spy_return_20d,
                                "relative_performance": relative_perf,
                                "signal": rotation_signal,
                            }),
                        );
                    }
                }
            }
        }

        (serde_json::Value::Object(signals), score_adj)
    }

    fn generate_recommendation(&self, signal: &SignalStrength, confidence: f64) -> String {
        let action = match signal {
            SignalStrength::StrongBuy => "Strong Buy",
            SignalStrength::Buy => "Buy",
            SignalStrength::WeakBuy => "Weak Buy / Hold",
            SignalStrength::Neutral => "Hold",
            SignalStrength::WeakSell => "Weak Sell / Hold",
            SignalStrength::Sell => "Sell",
            SignalStrength::StrongSell => "Strong Sell",
        };

        let confidence_desc = if confidence > 0.8 {
            "high"
        } else if confidence > 0.6 {
            "moderate"
        } else if confidence > 0.4 {
            "low"
        } else {
            "very low"
        };

        format!(
            "{} (confidence: {} - {:.0}%)",
            action,
            confidence_desc,
            confidence * 100.0
        )
    }

    /// Get historical bars for a symbol (cached, 5-min TTL)
    pub async fn get_bars(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        days_back: i64,
    ) -> Result<Vec<Bar>, AnalysisError> {
        let (multiplier, span) = match timeframe {
            Timeframe::Minute1 => (1, "minute"),
            Timeframe::Minute5 => (5, "minute"),
            Timeframe::Minute15 => (15, "minute"),
            Timeframe::Minute30 => (30, "minute"),
            Timeframe::Hour1 => (1, "hour"),
            Timeframe::Hour4 => (4, "hour"),
            Timeframe::Day1 => (1, "day"),
            Timeframe::Week1 => (1, "week"),
            Timeframe::Month1 => (1, "month"),
        };

        let cache_key = format!("{}:{}:{}:{}", symbol, multiplier, span, days_back);
        if let Some(entry) = self.bars_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        // Fast superset lookup via secondary index instead of scanning all cache entries.
        // E.g. a request for 30 days can be served from a cached 90-day entry.
        let prefix = format!("{}:{}:{}", symbol, multiplier, span);
        if let Some(cached_days_list) = self.bars_days_index.get(&prefix) {
            for &cached_days in cached_days_list.value() {
                if cached_days >= days_back {
                    let superset_key = format!("{}:{}", prefix, cached_days);
                    if let Some(entry) = self.bars_cache.get(&superset_key) {
                        let age = (Utc::now() - entry.cached_at).num_seconds();
                        if age < CACHE_TTL_SECS {
                            let cutoff = Utc::now() - Duration::days(days_back);
                            let subset: Vec<Bar> = entry
                                .data
                                .iter()
                                .filter(|b| b.timestamp >= cutoff)
                                .cloned()
                                .collect();
                            return Ok(subset);
                        }
                    }
                }
            }
        }

        let now = Utc::now();
        let start = now - Duration::days(days_back);
        let bars = self
            .polygon_client
            .get_aggregates(symbol, multiplier, span, start, now)
            .await?;

        self.bars_cache.insert(
            cache_key.clone(),
            CacheEntry {
                data: bars.clone(),
                cached_at: Utc::now(),
            },
        );

        // Update the secondary index
        self.bars_days_index
            .entry(prefix)
            .or_default()
            .push(days_back);

        Ok(bars)
    }

    /// Get ticker details (cached, 5-min TTL)
    pub async fn get_ticker_details(&self, symbol: &str) -> Result<TickerDetails, AnalysisError> {
        let cache_key = symbol.to_uppercase();
        if let Some(entry) = self.ticker_details_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        let details = self.polygon_client.get_ticker_details(symbol).await?;

        self.ticker_details_cache.insert(
            cache_key,
            CacheEntry {
                data: details.clone(),
                cached_at: Utc::now(),
            },
        );

        Ok(details)
    }

    /// Get company financials (cached, 5-min TTL)
    pub async fn get_financials(&self, symbol: &str) -> Result<Vec<Financials>, AnalysisError> {
        let cache_key = symbol.to_uppercase();
        if let Some(entry) = self.financials_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        let financials = self.polygon_client.get_financials(symbol).await?;

        self.financials_cache.insert(
            cache_key,
            CacheEntry {
                data: financials.clone(),
                cached_at: Utc::now(),
            },
        );

        Ok(financials)
    }

    /// Get dividend history
    pub async fn get_dividends(
        &self,
        symbol: &str,
    ) -> Result<Vec<polygon_client::DividendInfo>, AnalysisError> {
        self.polygon_client.get_dividends(symbol, 20).await
    }

    /// Get options chain snapshot
    pub async fn get_options_snapshot(
        &self,
        symbol: &str,
    ) -> Result<Vec<polygon_client::OptionsContractSnapshot>, AnalysisError> {
        self.polygon_client.get_options_snapshot(symbol).await
    }

    /// Get insider transactions
    pub async fn get_insider_transactions(
        &self,
        symbol: &str,
    ) -> Result<Vec<polygon_client::InsiderTransaction>, AnalysisError> {
        self.polygon_client
            .get_insider_transactions(symbol, 50)
            .await
    }

    /// Get news articles for a symbol (cached, 5-min TTL)
    pub async fn get_news(
        &self,
        symbol: &str,
        limit: u32,
    ) -> Result<Vec<NewsArticle>, AnalysisError> {
        let cache_key = format!("news:{}:{}", symbol, limit);
        if let Some(entry) = self.news_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        let articles = self.polygon_client.get_news(Some(symbol), limit).await?;

        self.news_cache.insert(
            cache_key,
            CacheEntry {
                data: articles.clone(),
                cached_at: Utc::now(),
            },
        );

        Ok(articles)
    }

    /// Get analyst consensus data (cached, 5-min TTL).
    /// Fetches both consensus ratings and recent individual ratings sequentially.
    /// Returns empty data on any error (graceful degradation).
    pub async fn get_analyst_consensus(&self, symbol: &str) -> AnalystConsensusData {
        let cache_key = symbol.to_uppercase();
        if let Some(entry) = self.consensus_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return entry.data.clone();
            }
        }

        let consensus = match self.polygon_client.get_consensus_ratings(symbol).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to fetch consensus ratings for {}: {:?}", symbol, e);
                None
            }
        };

        let recent_ratings = match self.polygon_client.get_analyst_ratings(symbol, 20).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to fetch analyst ratings for {}: {:?}", symbol, e);
                Vec::new()
            }
        };

        let data = AnalystConsensusData {
            consensus,
            recent_ratings,
        };

        self.consensus_cache.insert(
            cache_key,
            CacheEntry {
                data: data.clone(),
                cached_at: Utc::now(),
            },
        );

        data
    }
}
