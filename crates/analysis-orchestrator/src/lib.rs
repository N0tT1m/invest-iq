use analysis_core::{
    AnalysisError, AnalysisResult, AnalystConsensusData, Bar, Financials, NewsArticle,
    SignalStrength, Timeframe, UnifiedAnalysis, SentimentAnalyzer,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use polygon_client::{PolygonClient, TickerDetails};
use fundamental_analysis::FundamentalAnalysisEngine;
use quant_analysis::QuantAnalysisEngine;
use sentiment_analysis::SentimentAnalysisEngine;
use technical_analysis::TechnicalAnalysisEngine;
use ml_client::SignalModelsClient;
use std::collections::HashMap;

pub mod screener;
pub use screener::{StockScreener, StockSuggestion, ScreenerResult, StockUniverse, ScreenerFilters};

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
    /// Optional SQLite pool for logging analysis features
    db_pool: Option<sqlx::SqlitePool>,
    /// Cache news articles per symbol (5-min TTL)
    news_cache: DashMap<String, CacheEntry<Vec<NewsArticle>>>,
    /// Cache bars per (symbol, timeframe_key, days) (5-min TTL)
    bars_cache: DashMap<String, CacheEntry<Vec<Bar>>>,
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
            ticker_details_cache: DashMap::new(),
            financials_cache: DashMap::new(),
            consensus_cache: DashMap::new(),
        }
    }

    /// Set the SQLite pool for logging analysis features
    pub fn with_db_pool(mut self, pool: sqlx::SqlitePool) -> Self {
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

    /// Detect market regime from SPY bars using recent vs full-period volatility
    fn detect_market_regime(&self, spy_bars: &[Bar]) -> String {
        if spy_bars.len() < 20 {
            return "unknown".to_string();
        }

        let returns: Vec<f64> = spy_bars
            .windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();

        if returns.len() < 20 {
            return "unknown".to_string();
        }

        // Full-period volatility
        let full_mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let full_var = returns.iter().map(|r| (r - full_mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let full_vol = full_var.sqrt();

        // Recent 10-day volatility
        let recent = &returns[returns.len() - 10..];
        let recent_mean = recent.iter().sum::<f64>() / recent.len() as f64;
        let recent_var = recent.iter().map(|r| (r - recent_mean).powi(2)).sum::<f64>() / recent.len() as f64;
        let recent_vol = recent_var.sqrt();

        if full_vol == 0.0 {
            return "normal".to_string();
        }

        let ratio = recent_vol / full_vol;
        if ratio > 1.5 {
            "high_volatility".to_string()
        } else if ratio < 0.6 {
            "low_volatility".to_string()
        } else {
            "normal".to_string()
        }
    }

    /// Perform comprehensive analysis on a symbol
    pub async fn analyze(&self, symbol: &str, timeframe: Timeframe, days_back: i64) -> Result<UnifiedAnalysis, AnalysisError> {
        tracing::info!("Starting comprehensive analysis for {} (timeframe: {:?}, days: {})", symbol, timeframe, days_back);

        // Fire all API calls concurrently — Starter plan supports ~100 req/sec.
        // Cached responses (SPY/TLT bars, repeat symbols) return instantly.
        let (bars_result, financials_result, news_result, ticker_details, spy_bars_result, tlt_bars_result, snapshot_result) = tokio::join!(
            self.get_bars(symbol, timeframe, days_back),
            self.get_financials(symbol),
            self.get_news(symbol, 50),
            self.get_ticker_details(symbol),
            self.get_bars("SPY", Timeframe::Day1, 365),
            self.get_bars("TLT", Timeframe::Day1, 90),
            self.polygon_client.get_snapshot(symbol),
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
            tracing::info!("Current price for {}: {} (source: {})", symbol, price,
                if snapshot_price.is_some() { "snapshot" } else { "bar close" });
        } else {
            tracing::warn!("No current price found for {} - bars_result error: {:?}", symbol, bars_result.as_ref().err());
        }

        let spy_bars_ok = spy_bars_result.ok();

        // Extract shares outstanding from ticker details for DCF model
        let shares_outstanding = ticker_details.as_ref().ok().and_then(|d| {
            d.weighted_shares_outstanding.or(d.share_class_shares_outstanding)
        });

        // Derive dynamic risk-free rate from TLT price
        let dynamic_risk_free_rate = tlt_bars_result.ok()
            .and_then(|tlt_bars| {
                if tlt_bars.len() >= 2 {
                    let first = tlt_bars.first().unwrap();
                    let last = tlt_bars.last().unwrap();
                    let tlt_return = (last.close - first.close) / first.close;
                    // TLT inversely tracks yields: if TLT fell, rates rose
                    let rate = (0.045 - tlt_return * 0.10).max(0.01).min(0.08);
                    tracing::info!("Dynamic risk-free rate from TLT: {:.3} (TLT return: {:.3})", rate, tlt_return);
                    Some(rate)
                } else {
                    None
                }
            });

        // Run analyses using enhanced methods
        let mut technical_result = None;
        let mut quant_result = None;

        if let Ok(bars) = &bars_result {
            if bars.len() >= 50 {
                tracing::info!("Running enhanced technical analysis with {} bars", bars.len());
                match self.technical_analyzer.analyze_enhanced(symbol, bars) {
                    Ok(result) => technical_result = Some(result),
                    Err(e) => tracing::warn!("Technical analysis failed: {:?}", e),
                }
            }

            if bars.len() >= 30 {
                tracing::info!("Running enhanced quantitative analysis");
                match self.quant_analyzer.analyze_with_benchmark_and_rate(
                    symbol,
                    bars,
                    spy_bars_ok.as_deref(),
                    dynamic_risk_free_rate,
                ) {
                    Ok(result) => quant_result = Some(result),
                    Err(e) => tracing::warn!("Quant analysis failed: {:?}", e),
                }
            }
        }

        // Fetch analyst consensus (sequential to avoid rate limit pressure)
        let consensus_data = self.get_analyst_consensus(symbol).await;

        let mut fundamental_result = None;
        if let Ok(financials_vec) = &financials_result {
            if !financials_vec.is_empty() {
                tracing::info!("Running enhanced fundamental analysis with consensus data");
                match self.fundamental_analyzer.analyze_with_consensus(
                    symbol, financials_vec, current_price, shares_outstanding, &consensus_data, dynamic_risk_free_rate,
                ) {
                    Ok(result) => fundamental_result = Some(result),
                    Err(e) => tracing::warn!("Fundamental analysis failed: {:?}", e),
                }
            }
        }

        let mut sentiment_result = None;
        if let Ok(news) = &news_result {
            tracing::info!("Running sentiment analysis with {} articles", news.len());
            if let Ok(result) = self.sentiment_analyzer.analyze(symbol, news).await {
                sentiment_result = Some(result);
            }
        }

        // Combine results (now async — may fetch dynamic weights from ML service)
        let mut overall = self.combine_results(
            symbol,
            &technical_result,
            &fundamental_result,
            &quant_result,
            &sentiment_result,
        ).await;
        overall.current_price = current_price;
        overall.name = ticker_details.ok().map(|d| d.name);

        // Detect market regime from SPY bars
        if let Some(spy_bars) = &spy_bars_ok {
            overall.market_regime = Some(self.detect_market_regime(spy_bars));
        }

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
        );

        Ok(overall)
    }

    /// Combine individual analysis results into unified analysis.
    /// Optionally uses ML-predicted dynamic weights if the signal models service is available.
    async fn combine_results(
        &self,
        symbol: &str,
        technical: &Option<AnalysisResult>,
        fundamental: &Option<AnalysisResult>,
        quantitative: &Option<AnalysisResult>,
        sentiment: &Option<AnalysisResult>,
    ) -> UnifiedAnalysis {
        // Try to get dynamic weights from signal models service
        let dynamic_weights = self.try_get_dynamic_weights(technical, fundamental, quantitative, sentiment).await;

        // Use dynamic weights if available, otherwise fallback to hardcoded
        let (w_tech, w_fund, w_quant, w_sent) = match &dynamic_weights {
            Some(w) => {
                let wt = (w.get("technical").copied().unwrap_or(0.20) * 100.0) as i32;
                let wf = (w.get("fundamental").copied().unwrap_or(0.40) * 100.0) as i32;
                let wq = (w.get("quantitative").copied().unwrap_or(0.15) * 100.0) as i32;
                let ws = (w.get("sentiment").copied().unwrap_or(0.25) * 100.0) as i32;
                (wt, wf, wq, ws)
            }
            None => (20, 40, 15, 25),
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
        if let Some(tech) = technical { scores.push(tech.signal.to_score()); }
        if let Some(fund) = fundamental { scores.push(fund.signal.to_score()); }
        if let Some(quant) = quantitative { scores.push(quant.signal.to_score()); }
        if let Some(sent) = sentiment { scores.push(sent.signal.to_score()); }

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

        let recommendation = self.generate_recommendation(&overall_signal, overall_confidence);

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
            market_regime: None,
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
            features.insert("fundamental_score".to_string(), fund.signal.to_score() as f64);
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
                tracing::info!("Using dynamic weights from signal models: {:?}", engine_weights.weights);
                Some(engine_weights.weights)
            }
            Err(e) => {
                tracing::debug!("Signal models unavailable, using default weights: {}", e);
                None
            }
        }
    }

    /// Extract feature vector from analysis results and log to DB (fire-and-forget).
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
            features.insert("fundamental_score".to_string(), fund.signal.to_score() as f64);
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

        let features_json = match serde_json::to_string(&features) {
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

        // Check if a cached superset exists (same symbol/timeframe, more days).
        // E.g. a request for 30 days can be served from a cached 90-day entry.
        let prefix = format!("{}:{}:{}:", symbol, multiplier, span);
        for entry in self.bars_cache.iter() {
            if entry.key().starts_with(&prefix) && entry.key() != &cache_key {
                let age = (Utc::now() - entry.value().cached_at).num_seconds();
                if age < CACHE_TTL_SECS {
                    if let Some(cached_days_str) = entry.key().strip_prefix(&prefix) {
                        if let Ok(cached_days) = cached_days_str.parse::<i64>() {
                            if cached_days >= days_back {
                                let cutoff = Utc::now() - Duration::days(days_back);
                                let subset: Vec<Bar> = entry.value().data.iter()
                                    .filter(|b| b.timestamp >= cutoff)
                                    .cloned()
                                    .collect();
                                return Ok(subset);
                            }
                        }
                    }
                }
            }
        }

        let now = Utc::now();
        let start = now - Duration::days(days_back);
        let bars = self.polygon_client
            .get_aggregates(symbol, multiplier, span, start, now)
            .await?;

        self.bars_cache.insert(cache_key, CacheEntry {
            data: bars.clone(),
            cached_at: Utc::now(),
        });

        Ok(bars)
    }

    /// Get ticker details (cached, 5-min TTL)
    pub async fn get_ticker_details(
        &self,
        symbol: &str,
    ) -> Result<TickerDetails, AnalysisError> {
        let cache_key = symbol.to_uppercase();
        if let Some(entry) = self.ticker_details_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        let details = self.polygon_client.get_ticker_details(symbol).await?;

        self.ticker_details_cache.insert(cache_key, CacheEntry {
            data: details.clone(),
            cached_at: Utc::now(),
        });

        Ok(details)
    }

    /// Get company financials (cached, 5-min TTL)
    pub async fn get_financials(
        &self,
        symbol: &str,
    ) -> Result<Vec<Financials>, AnalysisError> {
        let cache_key = symbol.to_uppercase();
        if let Some(entry) = self.financials_cache.get(&cache_key) {
            let age = (Utc::now() - entry.cached_at).num_seconds();
            if age < CACHE_TTL_SECS {
                return Ok(entry.data.clone());
            }
        }

        let financials = self.polygon_client.get_financials(symbol).await?;

        self.financials_cache.insert(cache_key, CacheEntry {
            data: financials.clone(),
            cached_at: Utc::now(),
        });

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
        self.polygon_client.get_insider_transactions(symbol, 50).await
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

        self.news_cache.insert(cache_key, CacheEntry {
            data: articles.clone(),
            cached_at: Utc::now(),
        });

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

        self.consensus_cache.insert(cache_key, CacheEntry {
            data: data.clone(),
            cached_at: Utc::now(),
        });

        data
    }
}
