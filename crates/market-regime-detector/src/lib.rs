use analysis_core::Bar;
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::warn;

/// Market regime classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    /// Strong upward trend with low volatility
    TrendingBullish,

    /// Strong downward trend with low volatility
    TrendingBearish,

    /// Sideways movement with clear support/resistance
    Ranging,

    /// High volatility with rapid price swings
    Volatile,

    /// Low volatility, tight price range
    Calm,

    /// Unable to classify (insufficient data)
    Unknown,
}

impl MarketRegime {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            MarketRegime::TrendingBullish => "Trending Bullish",
            MarketRegime::TrendingBearish => "Trending Bearish",
            MarketRegime::Ranging => "Ranging",
            MarketRegime::Volatile => "Volatile",
            MarketRegime::Calm => "Calm",
            MarketRegime::Unknown => "Unknown",
        }
    }

    /// Get recommended strategies for this regime
    pub fn recommended_strategies(&self) -> Vec<&'static str> {
        match self {
            MarketRegime::TrendingBullish => vec!["momentum", "breakout", "trend_following"],
            MarketRegime::TrendingBearish => vec!["short_selling", "inverse_momentum"],
            MarketRegime::Ranging => vec!["mean_reversion", "range_trading", "support_resistance"],
            MarketRegime::Volatile => vec!["options", "small_positions", "tight_stops"],
            MarketRegime::Calm => vec!["position_building", "swing_trading"],
            MarketRegime::Unknown => vec!["conservative", "wait"],
        }
    }

    /// Get risk multiplier for this regime (1.0 = normal risk)
    pub fn risk_multiplier(&self) -> f64 {
        match self {
            MarketRegime::TrendingBullish | MarketRegime::TrendingBearish => 1.2,
            MarketRegime::Ranging => 1.0,
            MarketRegime::Volatile => 0.5,  // Reduce risk in volatile markets
            MarketRegime::Calm => 1.1,
            MarketRegime::Unknown => 0.3,   // Very conservative
        }
    }
}

/// Regime detection result with confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeDetectionResult {
    pub regime: MarketRegime,
    pub confidence: f64,
    pub metrics: RegimeMetrics,
    pub detected_at: DateTime<Utc>,
    pub reasoning: String,
}

/// Market regime metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeMetrics {
    /// Average True Range (ATR) as percentage
    pub atr_percent: f64,

    /// Trend strength (-1.0 to 1.0, negative = bearish, positive = bullish)
    pub trend_strength: f64,

    /// Volatility (standard deviation of returns)
    pub volatility: f64,

    /// Range efficiency (directional movement / total movement)
    pub range_efficiency: f64,

    /// Number of bars analyzed
    pub sample_size: usize,
}

/// Market regime detector
pub struct MarketRegimeDetector {
    /// URL to Python ML service (optional)
    ml_service_url: Option<String>,

    /// HTTP client for ML service
    client: Client,

    /// Minimum bars required for analysis
    min_bars: usize,
}

impl MarketRegimeDetector {
    pub fn new() -> Self {
        Self {
            ml_service_url: None,
            client: Client::new(),
            min_bars: 50,
        }
    }

    /// Create detector with ML service integration
    pub fn with_ml_service(url: String) -> Self {
        Self {
            ml_service_url: Some(url),
            client: Client::new(),
            min_bars: 50,
        }
    }

    /// Detect market regime using rule-based approach
    pub fn detect_regime(&self, bars: &[Bar]) -> Result<RegimeDetectionResult> {
        if bars.len() < self.min_bars {
            return Ok(RegimeDetectionResult {
                regime: MarketRegime::Unknown,
                confidence: 0.0,
                metrics: RegimeMetrics {
                    atr_percent: 0.0,
                    trend_strength: 0.0,
                    volatility: 0.0,
                    range_efficiency: 0.0,
                    sample_size: bars.len(),
                },
                detected_at: Utc::now(),
                reasoning: format!("Insufficient data: {} bars (need {})", bars.len(), self.min_bars),
            });
        }

        // Calculate metrics
        let metrics = self.calculate_metrics(bars);

        // Rule-based classification
        let (regime, confidence, reasoning) = self.classify_regime(&metrics);

        Ok(RegimeDetectionResult {
            regime,
            confidence,
            metrics,
            detected_at: Utc::now(),
            reasoning,
        })
    }

    /// Detect regime using ML model (if available), fallback to rule-based
    pub async fn detect_regime_ml(&self, bars: &[Bar]) -> Result<RegimeDetectionResult> {
        if let Some(url) = &self.ml_service_url {
            match self.query_ml_service(url, bars).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("ML service failed: {}. Falling back to rule-based detection.", e);
                }
            }
        }

        // Fallback to rule-based
        self.detect_regime(bars)
    }

    /// Calculate regime metrics
    fn calculate_metrics(&self, bars: &[Bar]) -> RegimeMetrics {
        let atr_percent = self.calculate_atr_percent(bars);
        let trend_strength = self.calculate_trend_strength(bars);
        let volatility = self.calculate_volatility(bars);
        let range_efficiency = self.calculate_range_efficiency(bars);

        RegimeMetrics {
            atr_percent,
            trend_strength,
            volatility,
            range_efficiency,
            sample_size: bars.len(),
        }
    }

    /// Calculate Average True Range as percentage of price
    fn calculate_atr_percent(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 14 {
            return 0.0;
        }

        let mut true_ranges = Vec::new();

        for i in 1..bars.len() {
            let high = bars[i].high;
            let low = bars[i].low;
            let prev_close = bars[i - 1].close;

            let tr = (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());

            true_ranges.push(tr);
        }

        // Average the last 14 true ranges
        let recent_trs = &true_ranges[true_ranges.len().saturating_sub(14)..];
        let atr: f64 = recent_trs.iter().sum::<f64>() / recent_trs.len() as f64;
        let current_price = bars.last().unwrap().close;

        (atr / current_price) * 100.0
    }

    /// Calculate trend strength using linear regression slope
    fn calculate_trend_strength(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 20 {
            return 0.0;
        }

        let _n = bars.len() as f64;
        let recent_bars = &bars[bars.len() - 20..];

        // Simple linear regression
        let sum_x: f64 = (0..20).sum::<usize>() as f64;
        let sum_y: f64 = recent_bars.iter().map(|b| b.close).sum();
        let sum_xy: f64 = recent_bars.iter().enumerate()
            .map(|(i, b)| i as f64 * b.close)
            .sum();
        let sum_x2: f64 = (0..20).map(|i| (i * i) as f64).sum();

        let slope = (20.0 * sum_xy - sum_x * sum_y) / (20.0 * sum_x2 - sum_x * sum_x);
        let avg_price: f64 = sum_y / 20.0;

        // Normalize slope by average price
        slope / avg_price
    }

    /// Calculate volatility (standard deviation of returns)
    fn calculate_volatility(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 2 {
            return 0.0;
        }

        let returns: Vec<f64> = bars.windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }

    /// Calculate range efficiency (how efficiently price moves)
    fn calculate_range_efficiency(&self, bars: &[Bar]) -> f64 {
        if bars.len() < 2 {
            return 0.0;
        }

        let first_price = bars.first().unwrap().close;
        let last_price = bars.last().unwrap().close;
        let net_movement = (last_price - first_price).abs();

        let total_movement: f64 = bars.windows(2)
            .map(|w| (w[1].close - w[0].close).abs())
            .sum();

        if total_movement == 0.0 {
            return 0.0;
        }

        net_movement / total_movement
    }

    /// Classify regime based on metrics
    fn classify_regime(&self, metrics: &RegimeMetrics) -> (MarketRegime, f64, String) {
        let mut scores = vec![
            (MarketRegime::TrendingBullish, 0.0),
            (MarketRegime::TrendingBearish, 0.0),
            (MarketRegime::Ranging, 0.0),
            (MarketRegime::Volatile, 0.0),
            (MarketRegime::Calm, 0.0),
        ];

        // High volatility
        if metrics.volatility > 0.03 {
            scores[3].1 += 40.0; // Volatile
        }

        // Low volatility
        if metrics.volatility < 0.01 {
            scores[4].1 += 30.0; // Calm
        }

        // Strong uptrend
        if metrics.trend_strength > 0.01 && metrics.range_efficiency > 0.5 {
            scores[0].1 += 50.0; // Trending Bullish
        }

        // Strong downtrend
        if metrics.trend_strength < -0.01 && metrics.range_efficiency > 0.5 {
            scores[1].1 += 50.0; // Trending Bearish
        }

        // Ranging (low efficiency, moderate volatility)
        if metrics.range_efficiency < 0.3 && metrics.volatility < 0.025 {
            scores[2].1 += 40.0; // Ranging
        }

        // ATR contribution
        if metrics.atr_percent > 3.0 {
            scores[3].1 += 20.0; // Volatile
        } else if metrics.atr_percent < 1.0 {
            scores[4].1 += 20.0; // Calm
        }

        // Find highest scoring regime
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let (regime, score) = scores[0];

        let confidence = f64::min(score / 100.0, 1.0);

        let reasoning = format!(
            "{} (trend: {:.3}, volatility: {:.2}%, efficiency: {:.2}, ATR: {:.2}%)",
            regime.name(),
            metrics.trend_strength,
            metrics.volatility * 100.0,
            metrics.range_efficiency,
            metrics.atr_percent
        );

        (regime, confidence, reasoning)
    }

    /// Query Python ML service for regime detection
    async fn query_ml_service(&self, url: &str, bars: &[Bar]) -> Result<RegimeDetectionResult> {
        #[derive(Serialize)]
        struct MLRequest {
            bars: Vec<MLBar>,
        }

        #[derive(Serialize)]
        struct MLBar {
            timestamp: i64,
            open: f64,
            high: f64,
            low: f64,
            close: f64,
            volume: f64,
        }

        #[derive(Deserialize)]
        struct MLResponse {
            regime: String,
            confidence: f64,
            reasoning: String,
        }

        let request = MLRequest {
            bars: bars.iter().map(|b| MLBar {
                timestamp: b.timestamp.timestamp(),
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume,
            }).collect(),
        };

        let response = self.client
            .post(format!("{}/detect_regime", url))
            .json(&request)
            .send()
            .await?;

        let ml_response: MLResponse = response.json().await?;

        let regime = match ml_response.regime.as_str() {
            "trending_bullish" => MarketRegime::TrendingBullish,
            "trending_bearish" => MarketRegime::TrendingBearish,
            "ranging" => MarketRegime::Ranging,
            "volatile" => MarketRegime::Volatile,
            "calm" => MarketRegime::Calm,
            _ => MarketRegime::Unknown,
        };

        let metrics = self.calculate_metrics(bars);

        Ok(RegimeDetectionResult {
            regime,
            confidence: ml_response.confidence,
            metrics,
            detected_at: Utc::now(),
            reasoning: ml_response.reasoning,
        })
    }
}

impl Default for MarketRegimeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_bars(count: usize, trend: f64) -> Vec<Bar> {
        (0..count)
            .map(|i| {
                let base_price = 100.0 + (i as f64 * trend);
                Bar {
                    timestamp: Utc::now(),
                    open: base_price,
                    high: base_price + 1.0,
                    low: base_price - 1.0,
                    close: base_price,
                    volume: 1000.0,
                    vwap: None,
                }
            })
            .collect()
    }

    #[test]
    fn test_uptrend_detection() {
        let detector = MarketRegimeDetector::new();
        let bars = create_test_bars(100, 0.5); // Uptrend

        let result = detector.detect_regime(&bars).unwrap();

        assert!(matches!(result.regime, MarketRegime::TrendingBullish));
        assert!(result.metrics.trend_strength > 0.0);
    }

    #[test]
    fn test_downtrend_detection() {
        let detector = MarketRegimeDetector::new();
        let bars = create_test_bars(100, -0.5); // Downtrend

        let result = detector.detect_regime(&bars).unwrap();

        assert!(matches!(result.regime, MarketRegime::TrendingBearish));
        assert!(result.metrics.trend_strength < 0.0);
    }

    #[test]
    fn test_insufficient_data() {
        let detector = MarketRegimeDetector::new();
        let bars = create_test_bars(10, 0.0); // Too few bars

        let result = detector.detect_regime(&bars).unwrap();

        assert_eq!(result.regime, MarketRegime::Unknown);
        assert_eq!(result.confidence, 0.0);
    }
}
