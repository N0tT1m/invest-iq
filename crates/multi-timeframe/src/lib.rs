use analysis_core::{AnalysisError, Bar};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use log::debug;
use polygon_client::PolygonClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported trading timeframes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    /// 5-minute bars
    Min5,
    /// 15-minute bars
    Min15,
    /// 1-hour bars
    Hour1,
    /// 4-hour bars
    Hour4,
    /// Daily bars
    Daily,
}

impl Timeframe {
    /// Get the multiplier and timespan for Polygon API
    pub fn to_polygon_params(&self) -> (u32, &'static str) {
        match self {
            Timeframe::Min5 => (5, "minute"),
            Timeframe::Min15 => (15, "minute"),
            Timeframe::Hour1 => (1, "hour"),
            Timeframe::Hour4 => (4, "hour"),
            Timeframe::Daily => (1, "day"),
        }
    }

    /// Get the duration represented by one bar
    pub fn to_duration(&self) -> Duration {
        match self {
            Timeframe::Min5 => Duration::minutes(5),
            Timeframe::Min15 => Duration::minutes(15),
            Timeframe::Hour1 => Duration::hours(1),
            Timeframe::Hour4 => Duration::hours(4),
            Timeframe::Daily => Duration::days(1),
        }
    }

    /// Get the number of bars to fetch for a given lookback period
    pub fn bars_for_lookback(&self, days: i64) -> i64 {
        match self {
            Timeframe::Min5 => days * 78,  // ~6.5 hours * 12 bars/hour
            Timeframe::Min15 => days * 26, // ~6.5 hours * 4 bars/hour
            Timeframe::Hour1 => days * 7,  // ~6.5 hours/day
            Timeframe::Hour4 => days * 2,  // ~2 bars per day
            Timeframe::Daily => days,      // 1 bar per day
        }
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Timeframe::Min5 => "5min",
            Timeframe::Min15 => "15min",
            Timeframe::Hour1 => "1hour",
            Timeframe::Hour4 => "4hour",
            Timeframe::Daily => "daily",
        }
    }

    /// All available timeframes
    pub fn all() -> Vec<Timeframe> {
        vec![
            Timeframe::Min5,
            Timeframe::Min15,
            Timeframe::Hour1,
            Timeframe::Hour4,
            Timeframe::Daily,
        ]
    }
}

/// Multi-timeframe data for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTimeframeData {
    pub symbol: String,
    pub data: HashMap<Timeframe, Vec<Bar>>,
    pub last_updated: DateTime<Utc>,
}

/// Multi-timeframe signal with trend alignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTimeframeSignal {
    pub symbol: String,
    pub timeframe: Timeframe,
    pub signal_type: SignalType,
    pub trend_alignment: TrendAlignment,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
    Neutral,
}

/// Trend alignment across multiple timeframes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAlignment {
    /// Trends by timeframe (true = uptrend, false = downtrend)
    pub trends: HashMap<Timeframe, bool>,
    /// Percentage of timeframes in agreement
    pub alignment_score: f64,
    /// Overall trend direction
    pub overall_trend: SignalType,
}

/// Multi-timeframe analyzer
pub struct MultiTimeframeAnalyzer {
    client: PolygonClient,
    lookback_days: i64,
}

impl MultiTimeframeAnalyzer {
    pub fn new(api_key: String) -> Self {
        Self {
            client: PolygonClient::new(api_key),
            lookback_days: 30,
        }
    }

    pub fn with_lookback(api_key: String, lookback_days: i64) -> Self {
        Self {
            client: PolygonClient::new(api_key),
            lookback_days,
        }
    }

    /// Fetch data for all timeframes concurrently using join_all
    pub async fn fetch_all_timeframes(&self, symbol: &str) -> Result<MultiTimeframeData> {
        let to = Utc::now();
        let from = to - Duration::days(self.lookback_days);

        // Fetch all 5 timeframes concurrently
        let futures: Vec<_> = Timeframe::all()
            .into_iter()
            .map(|timeframe| {
                let client = &self.client;
                let symbol = symbol.to_string();
                let (multiplier, timespan) = timeframe.to_polygon_params();
                async move {
                    debug!("Fetching {} data for {}", timeframe.name(), symbol);
                    match client
                        .get_aggregates(&symbol, multiplier, timespan, from, to)
                        .await
                    {
                        Ok(bars) if !bars.is_empty() => Some((timeframe, bars)),
                        Ok(_) => {
                            debug!("No data for {} on {}", symbol, timeframe.name());
                            None
                        }
                        Err(e) => {
                            debug!("Failed to fetch {} data: {}", timeframe.name(), e);
                            None
                        }
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let data: HashMap<Timeframe, Vec<Bar>> = results.into_iter().flatten().collect();

        Ok(MultiTimeframeData {
            symbol: symbol.to_string(),
            data,
            last_updated: Utc::now(),
        })
    }

    /// Fetch data for specific timeframes concurrently
    pub async fn fetch_timeframes(
        &self,
        symbol: &str,
        timeframes: &[Timeframe],
    ) -> Result<MultiTimeframeData> {
        let to = Utc::now();
        let from = to - Duration::days(self.lookback_days);

        let futures: Vec<_> = timeframes
            .iter()
            .map(|&timeframe| {
                let client = &self.client;
                let symbol = symbol.to_string();
                let (multiplier, timespan) = timeframe.to_polygon_params();
                async move {
                    match client
                        .get_aggregates(&symbol, multiplier, timespan, from, to)
                        .await
                    {
                        Ok(bars) if !bars.is_empty() => Some((timeframe, bars)),
                        Ok(_) => None,
                        Err(e) => {
                            debug!("Failed to fetch {} data: {}", timeframe.name(), e);
                            None
                        }
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let data: HashMap<Timeframe, Vec<Bar>> = results.into_iter().flatten().collect();

        Ok(MultiTimeframeData {
            symbol: symbol.to_string(),
            data,
            last_updated: Utc::now(),
        })
    }

    /// Analyze trend alignment across timeframes
    pub fn analyze_trend_alignment(&self, mtf_data: &MultiTimeframeData) -> Result<TrendAlignment> {
        let mut trends = HashMap::new();

        for (timeframe, bars) in &mtf_data.data {
            if bars.len() < 20 {
                continue; // Need at least 20 bars for trend analysis
            }

            // Simple trend detection using moving averages
            let trend = self.detect_trend(bars);
            trends.insert(*timeframe, trend);
        }

        if trends.is_empty() {
            return Ok(TrendAlignment {
                trends,
                alignment_score: 0.0,
                overall_trend: SignalType::Neutral,
            });
        }

        // Calculate alignment score
        let uptrend_count = trends.values().filter(|&&t| t).count() as f64;
        let total_count = trends.len() as f64;
        let alignment_score = (uptrend_count / total_count - 0.5).abs() * 2.0;

        // Determine overall trend
        let overall_trend = if uptrend_count / total_count > 0.6 {
            SignalType::Buy
        } else if uptrend_count / total_count < 0.4 {
            SignalType::Sell
        } else {
            SignalType::Neutral
        };

        Ok(TrendAlignment {
            trends,
            alignment_score,
            overall_trend,
        })
    }

    /// Detect trend using simple moving averages
    fn detect_trend(&self, bars: &[Bar]) -> bool {
        if bars.len() < 20 {
            return false;
        }

        // Calculate 10-period and 20-period SMAs
        let sma10 = self.calculate_sma(bars, 10);
        let sma20 = self.calculate_sma(bars, 20);

        // Uptrend if short-term MA > long-term MA
        sma10 > sma20
    }

    /// Calculate Simple Moving Average
    fn calculate_sma(&self, bars: &[Bar], period: usize) -> f64 {
        if bars.len() < period {
            return 0.0;
        }

        let recent_bars = &bars[bars.len() - period..];
        let sum: f64 = recent_bars.iter().map(|b| b.close).sum();
        sum / period as f64
    }

    /// Generate trading signal based on multi-timeframe analysis
    pub fn generate_signal(
        &self,
        mtf_data: &MultiTimeframeData,
        primary_timeframe: Timeframe,
    ) -> Result<MultiTimeframeSignal> {
        let alignment = self.analyze_trend_alignment(mtf_data)?;

        // Get primary timeframe data
        let primary_bars = mtf_data.data.get(&primary_timeframe).ok_or_else(|| {
            AnalysisError::InvalidData(format!(
                "No data for primary timeframe: {}",
                primary_timeframe.name()
            ))
        })?;

        // Determine signal based on primary timeframe and alignment
        let primary_trend = self.detect_trend(primary_bars);

        let (signal_type, confidence, reasoning) = match (&alignment.overall_trend, primary_trend) {
            (SignalType::Buy, true) => {
                let conf = 0.7 + (alignment.alignment_score * 0.3);
                (
                    SignalType::Buy,
                    conf,
                    format!(
                        "Strong buy: {} timeframes aligned bullish ({:.0}% alignment)",
                        alignment.trends.len(),
                        alignment.alignment_score * 100.0
                    ),
                )
            }
            (SignalType::Sell, false) => {
                let conf = 0.7 + (alignment.alignment_score * 0.3);
                (
                    SignalType::Sell,
                    conf,
                    format!(
                        "Strong sell: {} timeframes aligned bearish ({:.0}% alignment)",
                        alignment.trends.len(),
                        alignment.alignment_score * 100.0
                    ),
                )
            }
            (SignalType::Buy, false) | (SignalType::Sell, true) => (
                SignalType::Neutral,
                0.3,
                "Conflicting signals between timeframes".to_string(),
            ),
            _ => (
                SignalType::Neutral,
                0.5,
                "Neutral market conditions".to_string(),
            ),
        };

        Ok(MultiTimeframeSignal {
            symbol: mtf_data.symbol.clone(),
            timeframe: primary_timeframe,
            signal_type,
            trend_alignment: alignment,
            confidence,
            reasoning,
        })
    }

    /// Find the best timeframe to trade based on trend strength
    pub fn find_best_timeframe(&self, mtf_data: &MultiTimeframeData) -> Option<Timeframe> {
        let mut best_timeframe = None;
        let mut best_score = 0.0;

        for (&timeframe, bars) in &mtf_data.data {
            if bars.len() < 20 {
                continue;
            }

            // Calculate trend strength (distance between SMAs relative to price)
            let sma10 = self.calculate_sma(bars, 10);
            let sma20 = self.calculate_sma(bars, 20);
            let current_price = bars.last().unwrap().close;

            let trend_strength = ((sma10 - sma20).abs() / current_price).abs();

            if trend_strength > best_score {
                best_score = trend_strength;
                best_timeframe = Some(timeframe);
            }
        }

        best_timeframe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_params() {
        assert_eq!(Timeframe::Min5.to_polygon_params(), (5, "minute"));
        assert_eq!(Timeframe::Hour1.to_polygon_params(), (1, "hour"));
        assert_eq!(Timeframe::Daily.to_polygon_params(), (1, "day"));
    }

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::Min5.to_duration(), Duration::minutes(5));
        assert_eq!(Timeframe::Hour1.to_duration(), Duration::hours(1));
        assert_eq!(Timeframe::Daily.to_duration(), Duration::days(1));
    }

    #[test]
    fn test_trend_detection() {
        let analyzer = MultiTimeframeAnalyzer::new("test_key".to_string());

        // Create uptrend data
        let uptrend_bars: Vec<Bar> = (0..30)
            .map(|i| Bar {
                timestamp: Utc::now(),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.0 + i as f64,
                volume: 1000.0,
                vwap: None,
            })
            .collect();

        assert!(analyzer.detect_trend(&uptrend_bars));

        // Create downtrend data
        let downtrend_bars: Vec<Bar> = (0..30)
            .map(|i| Bar {
                timestamp: Utc::now(),
                open: 130.0 - i as f64,
                high: 131.0 - i as f64,
                low: 129.0 - i as f64,
                close: 130.0 - i as f64,
                volume: 1000.0,
                vwap: None,
            })
            .collect();

        assert!(!analyzer.detect_trend(&downtrend_bars));
    }
}
