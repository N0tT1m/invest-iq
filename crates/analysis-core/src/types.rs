use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OHLCV bar data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(default)]
    pub vwap: Option<f64>,
}

/// Quote data (bid/ask)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub timestamp: DateTime<Utc>,
    pub bid: f64,
    pub ask: f64,
    pub bid_size: i64,
    pub ask_size: i64,
}

/// Trade data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub size: i64,
}

/// Company financials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Financials {
    pub symbol: String,
    pub fiscal_period: String,
    pub fiscal_year: i32,
    pub revenue: Option<f64>,
    pub gross_profit: Option<f64>,
    pub operating_income: Option<f64>,
    pub net_income: Option<f64>,
    pub eps: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub shareholders_equity: Option<f64>,
    pub cash_flow_operating: Option<f64>,
    pub cash_flow_investing: Option<f64>,
    pub cash_flow_financing: Option<f64>,
}

/// Analyst consensus rating (aggregated from multiple analysts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusRating {
    pub consensus_rating: Option<String>,
    pub consensus_price_target: Option<f64>,
    pub high_price_target: Option<f64>,
    pub low_price_target: Option<f64>,
    pub buy_count: Option<i32>,
    pub hold_count: Option<i32>,
    pub sell_count: Option<i32>,
    pub contributors: Option<i32>,
}

/// Individual analyst rating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalystRating {
    pub price_target: Option<f64>,
    pub rating: Option<String>,
    pub rating_action: Option<String>,
    pub analyst: Option<String>,
    pub firm: Option<String>,
    pub date: Option<String>,
}

/// Wrapper for analyst consensus data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalystConsensusData {
    pub consensus: Option<ConsensusRating>,
    pub recent_ratings: Vec<AnalystRating>,
}

/// News article
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub published_utc: DateTime<Utc>,
    pub article_url: String,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub tickers: Vec<String>,
}

/// Signal strength
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SignalStrength {
    StrongBuy,
    Buy,
    WeakBuy,
    Neutral,
    WeakSell,
    Sell,
    StrongSell,
}

impl SignalStrength {
    /// Convert to numeric score (-100 to 100)
    pub fn to_score(&self) -> i32 {
        match self {
            SignalStrength::StrongBuy => 100,
            SignalStrength::Buy => 60,
            SignalStrength::WeakBuy => 30,
            SignalStrength::Neutral => 0,
            SignalStrength::WeakSell => -30,
            SignalStrength::Sell => -60,
            SignalStrength::StrongSell => -100,
        }
    }

    pub fn from_score(score: i32) -> Self {
        match score {
            s if s >= 70 => SignalStrength::StrongBuy,
            s if s >= 30 => SignalStrength::Buy,
            s if s >= 5 => SignalStrength::WeakBuy,
            s if s >= -5 => SignalStrength::Neutral,
            s if s >= -30 => SignalStrength::WeakSell,
            s if s >= -70 => SignalStrength::Sell,
            _ => SignalStrength::StrongSell,
        }
    }

    /// Human-readable label for the signal
    pub fn to_label(&self) -> &'static str {
        match self {
            SignalStrength::StrongBuy => "Strong Buy",
            SignalStrength::Buy => "Buy",
            SignalStrength::WeakBuy => "Weak Buy",
            SignalStrength::Neutral => "Neutral",
            SignalStrength::WeakSell => "Weak Sell",
            SignalStrength::Sell => "Sell",
            SignalStrength::StrongSell => "Strong Sell",
        }
    }
}

/// Analysis result from any analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub signal: SignalStrength,
    pub confidence: f64, // 0.0 to 1.0
    pub reason: String,
    pub metrics: serde_json::Value,
}

/// Combined analysis from all engines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAnalysis {
    pub symbol: String,
    #[serde(default)]
    pub name: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub current_price: Option<f64>,
    pub technical: Option<AnalysisResult>,
    pub fundamental: Option<AnalysisResult>,
    pub quantitative: Option<AnalysisResult>,
    pub sentiment: Option<AnalysisResult>,
    pub overall_signal: SignalStrength,
    pub overall_confidence: f64,
    pub recommendation: String,
    #[serde(default)]
    pub market_regime: Option<String>,
    /// Conviction tier: HIGH, MODERATE, LOW based on engine alignment + confidence
    #[serde(default)]
    pub conviction_tier: Option<String>,
    /// Per-engine time horizon tags (short/medium/long-term signals)
    #[serde(default)]
    pub time_horizon_signals: Option<serde_json::Value>,
    /// Supplementary signals from options, insiders, dividends, etc.
    #[serde(default)]
    pub supplementary_signals: Option<serde_json::Value>,
}

/// Timeframe for analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Timeframe {
    Minute1,
    Minute5,
    Minute15,
    Minute30,
    Hour1,
    Hour4,
    Day1,
    Week1,
    Month1,
}

impl Timeframe {
    pub fn to_minutes(&self) -> i64 {
        match self {
            Timeframe::Minute1 => 1,
            Timeframe::Minute5 => 5,
            Timeframe::Minute15 => 15,
            Timeframe::Minute30 => 30,
            Timeframe::Hour1 => 60,
            Timeframe::Hour4 => 240,
            Timeframe::Day1 => 1440,
            Timeframe::Week1 => 10080,
            Timeframe::Month1 => 43200,
        }
    }
}
