//! Smart Watchlist Data Models

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Type of upcoming event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum EventType {
    Earnings,
    Dividend,
    FDA,
    Split,
    Conference,
    Other(String),
}

impl EventType {
    pub fn as_str(&self) -> &str {
        match self {
            EventType::Earnings => "Earnings",
            EventType::Dividend => "Dividend",
            EventType::FDA => "FDA",
            EventType::Split => "Split",
            EventType::Conference => "Conference",
            EventType::Other(s) => s,
        }
    }
}

/// Signal type for an opportunity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum OpportunitySignal {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

impl OpportunitySignal {
    pub fn score(&self) -> f64 {
        match self {
            OpportunitySignal::StrongBuy => 1.0,
            OpportunitySignal::Buy => 0.75,
            OpportunitySignal::Neutral => 0.5,
            OpportunitySignal::Sell => 0.25,
            OpportunitySignal::StrongSell => 0.0,
        }
    }

    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 0.9 => OpportunitySignal::StrongBuy,
            s if s >= 0.65 => OpportunitySignal::Buy,
            s if s >= 0.35 => OpportunitySignal::Neutral,
            s if s >= 0.1 => OpportunitySignal::Sell,
            _ => OpportunitySignal::StrongSell,
        }
    }
}

/// A trading opportunity detected by the scanner
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Opportunity {
    /// Stock symbol
    pub symbol: String,
    /// Company name
    pub name: Option<String>,
    /// Current signal
    pub signal: OpportunitySignal,
    /// Confidence in the signal (0-1)
    pub confidence: f64,
    /// Why this is an opportunity
    pub reason: String,
    /// Short description
    pub summary: String,
    /// Upcoming event if any
    pub event_type: Option<EventType>,
    /// Date of upcoming event
    pub event_date: Option<NaiveDate>,
    /// Personal relevance score (0-100)
    pub relevance_score: f64,
    /// Current price
    pub current_price: Option<f64>,
    /// Price target if available
    pub price_target: Option<f64>,
    /// Potential return percentage
    pub potential_return: Option<f64>,
    /// Sector
    pub sector: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// When this opportunity was detected
    pub detected_at: DateTime<Utc>,
    /// Expires when no longer relevant
    pub expires_at: Option<DateTime<Utc>>,
}

impl Opportunity {
    /// Check if the opportunity has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires
        } else {
            false
        }
    }

    /// Check if this is a high-priority opportunity
    pub fn is_high_priority(&self) -> bool {
        self.relevance_score >= 75.0
            && (self.signal == OpportunitySignal::StrongBuy
                || self.signal == OpportunitySignal::StrongSell)
    }
}

/// User interaction type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionType {
    Click,
    Dismiss,
    WatchlistAdd,
    WatchlistRemove,
    Trade,
    Analyze,
}

impl InteractionType {
    pub fn as_str(&self) -> &str {
        match self {
            InteractionType::Click => "click",
            InteractionType::Dismiss => "dismiss",
            InteractionType::WatchlistAdd => "watchlist_add",
            InteractionType::WatchlistRemove => "watchlist_remove",
            InteractionType::Trade => "trade",
            InteractionType::Analyze => "analyze",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "click" => InteractionType::Click,
            "dismiss" => InteractionType::Dismiss,
            "watchlist_add" => InteractionType::WatchlistAdd,
            "watchlist_remove" => InteractionType::WatchlistRemove,
            "trade" => InteractionType::Trade,
            "analyze" => InteractionType::Analyze,
            _ => InteractionType::Click,
        }
    }

    /// Weight for learning preferences (positive = interested, negative = not interested)
    pub fn preference_weight(&self) -> f64 {
        match self {
            InteractionType::Trade => 5.0,
            InteractionType::WatchlistAdd => 3.0,
            InteractionType::Analyze => 2.0,
            InteractionType::Click => 1.0,
            InteractionType::Dismiss => -2.0,
            InteractionType::WatchlistRemove => -1.0,
        }
    }
}

/// A user interaction with a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInteraction {
    pub id: Option<i64>,
    pub user_id: String,
    pub symbol: String,
    pub action: InteractionType,
    pub context: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// User preferences for personalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub user_id: String,
    /// Preferred sectors (e.g., "Technology", "Healthcare")
    pub preferred_sectors: Vec<String>,
    /// Preferred market cap ranges
    pub preferred_market_caps: Vec<String>,
    /// Risk tolerance (0-1, 0=conservative, 1=aggressive)
    pub risk_tolerance: f64,
    /// Average holding period in days
    pub avg_hold_period_days: i32,
    /// Minimum confidence threshold (0-1)
    pub min_confidence: f64,
    /// Preferred signal types
    pub preferred_signals: Vec<String>,
    /// Symbols to exclude
    pub excluded_symbols: Vec<String>,
    /// Symbol affinities learned from interactions (symbol -> score)
    pub symbol_affinities: std::collections::HashMap<String, f64>,
    /// Sector affinities learned from interactions
    pub sector_affinities: std::collections::HashMap<String, f64>,
    /// When preferences were last updated
    pub updated_at: DateTime<Utc>,
}

impl Default for UserPreference {
    fn default() -> Self {
        Self {
            user_id: "default".to_string(),
            preferred_sectors: Vec::new(),
            preferred_market_caps: vec!["large".to_string(), "mid".to_string()],
            risk_tolerance: 0.5,
            avg_hold_period_days: 30,
            min_confidence: 0.5,
            preferred_signals: vec!["StrongBuy".to_string(), "Buy".to_string()],
            excluded_symbols: Vec::new(),
            symbol_affinities: std::collections::HashMap::new(),
            sector_affinities: std::collections::HashMap::new(),
            updated_at: Utc::now(),
        }
    }
}

/// An item in the user's watchlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: Option<i64>,
    pub user_id: String,
    pub symbol: String,
    pub added_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub target_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub alert_enabled: bool,
}
