use anyhow::Result;
use crate::config::AgentConfig;
use chrono::{Utc, Timelike, Datelike, Weekday};
use polygon_client::PolygonClient;
use multi_timeframe::{MultiTimeframeAnalyzer, Timeframe};
use market_regime_detector::{MarketRegimeDetector, MarketRegime};
use news_trading::NewsScanner;

#[derive(Debug, Clone)]
pub struct MarketOpportunity {
    pub symbol: String,
    pub current_price: f64,
    pub volume: i64,
    pub price_change_percent: f64,
    pub volatility: f64,
    pub regime: MarketRegime,
    pub primary_timeframe: Timeframe,
    pub news_sentiment_score: f64,
}

pub struct MarketScanner {
    config: AgentConfig,
    polygon_client: PolygonClient,
    mtf_analyzer: MultiTimeframeAnalyzer,
    regime_detector: MarketRegimeDetector,
    news_scanner: NewsScanner,
}

impl MarketScanner {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let polygon_client = PolygonClient::new(config.polygon_api_key.clone());
        let mtf_analyzer = MultiTimeframeAnalyzer::new(config.polygon_api_key.clone());

        let regime_detector = if let Some(url) = &config.regime_ml_service_url {
            MarketRegimeDetector::with_ml_service(url.clone())
        } else {
            MarketRegimeDetector::new()
        };

        let news_scanner = if let Some(url) = &config.news_sentiment_service_url {
            NewsScanner::with_sentiment_service(config.polygon_api_key.clone(), url.clone())
        } else {
            NewsScanner::new(config.polygon_api_key.clone())
        };

        Ok(Self {
            config,
            polygon_client,
            mtf_analyzer,
            regime_detector,
            news_scanner,
        })
    }

    pub async fn scan(&self) -> Result<Vec<MarketOpportunity>> {
        tracing::debug!("Scanning market for opportunities...");

        // Check if we should scan based on market hours
        if !self.should_scan_now() {
            tracing::info!("Outside trading hours. Skipping scan.");
            return Ok(Vec::new());
        }

        let mut opportunities = Vec::new();

        // 1. Scan watchlist
        for symbol in &self.config.watchlist {
            if let Ok(opp) = self.analyze_symbol(symbol).await {
                opportunities.push(opp);
            }
        }

        // 2. Scan top movers (if enabled)
        if self.config.scan_top_movers {
            if let Ok(mut top_movers) = self.scan_top_movers().await {
                opportunities.append(&mut top_movers);
            }
        }

        // 3. Remove duplicates
        opportunities.dedup_by(|a, b| a.symbol == b.symbol);

        // 4. Filter by volume and volatility
        let min_volume = if self.is_extended_hours() {
            self.config.extended_hours_min_volume
        } else {
            self.config.regular_hours_min_volume
        };

        opportunities.retain(|opp| {
            opp.volume > min_volume &&
            opp.volatility > 0.005      // Minimum volatility (0.5%)
        });

        tracing::info!("Market scan complete: {} opportunities found", opportunities.len());

        Ok(opportunities)
    }

    async fn analyze_symbol(&self, symbol: &str) -> Result<MarketOpportunity> {
        // Fetch multi-timeframe data
        let mtf_data = self.mtf_analyzer.fetch_all_timeframes(symbol).await?;

        // Find best timeframe
        let primary_timeframe = self.mtf_analyzer.find_best_timeframe(&mtf_data)
            .unwrap_or(Timeframe::Min15);

        // Get bars for regime detection
        let bars = mtf_data.data.get(&Timeframe::Daily)
            .or_else(|| mtf_data.data.get(&Timeframe::Hour1))
            .ok_or_else(|| anyhow::anyhow!("No data available for {}", symbol))?;

        // Detect market regime
        let regime_result = self.regime_detector.detect_regime(bars)?;

        // Analyze news sentiment
        let news_sentiment = self.news_scanner.analyze_aggregated(symbol, 24).await?;

        // Get current price and volume from latest bar
        let latest_bar = bars.last()
            .ok_or_else(|| anyhow::anyhow!("No bars available"))?;

        let current_price = latest_bar.close;
        let volume = latest_bar.volume as i64;

        // Calculate price change
        let price_change_percent = if bars.len() > 1 {
            let prev_close = bars[bars.len() - 2].close;
            ((current_price - prev_close) / prev_close) * 100.0
        } else {
            0.0
        };

        Ok(MarketOpportunity {
            symbol: symbol.to_string(),
            current_price,
            volume,
            price_change_percent,
            volatility: regime_result.metrics.volatility,
            regime: regime_result.regime,
            primary_timeframe,
            news_sentiment_score: news_sentiment.sentiment_score,
        })
    }

    async fn scan_top_movers(&self) -> Result<Vec<MarketOpportunity>> {
        // Scan for stocks with high volume and price movement
        // This would query Polygon's gainers/losers API
        // For now, returning empty
        Ok(Vec::new())
    }

    /// Check if we should scan based on market hours
    fn should_scan_now(&self) -> bool {
        let now = Utc::now().with_timezone(&chrono_tz::US::Eastern);

        // Skip weekends
        if now.weekday() == Weekday::Sat || now.weekday() == Weekday::Sun {
            return false;
        }

        let hour = now.hour();
        let minute = now.minute();
        let time_minutes = hour * 60 + minute;

        // Regular market hours: 9:30 AM - 4:00 PM ET
        let regular_open = 9 * 60 + 30;  // 9:30 AM
        let regular_close = 16 * 60;     // 4:00 PM

        // Extended hours: 4:00 AM - 9:30 AM, 4:00 PM - 8:00 PM ET
        let premarket_open = 4 * 60;     // 4:00 AM
        let afterhours_close = 20 * 60;  // 8:00 PM

        if time_minutes >= regular_open && time_minutes < regular_close {
            // Regular market hours
            return true;
        }

        if self.config.enable_extended_hours {
            // Pre-market: 4:00 AM - 9:30 AM
            if time_minutes >= premarket_open && time_minutes < regular_open {
                return true;
            }
            // After-hours: 4:00 PM - 8:00 PM
            if time_minutes >= regular_close && time_minutes < afterhours_close {
                return true;
            }
        }

        false
    }

    /// Check if we're in extended hours
    fn is_extended_hours(&self) -> bool {
        let now = Utc::now().with_timezone(&chrono_tz::US::Eastern);
        let hour = now.hour();
        let minute = now.minute();
        let time_minutes = hour * 60 + minute;

        let regular_open = 9 * 60 + 30;
        let regular_close = 16 * 60;

        time_minutes < regular_open || time_minutes >= regular_close
    }
}
