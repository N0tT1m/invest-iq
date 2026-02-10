//! Smart Watchlist Module
//!
//! AI-curated, personalized opportunity feed that learns user preferences.
//! Scans for opportunities and ranks them by personal relevance.

pub mod models;
pub mod preference_learner;
pub mod opportunity_scanner;
pub mod ranker;

pub use models::{
    EventType, Opportunity, OpportunitySignal, SymbolInteraction, UserPreference,
    InteractionType, WatchlistItem,
};
pub use preference_learner::PreferenceLearner;
pub use opportunity_scanner::{OpportunityScanner, ScanConfig};
pub use ranker::{OpportunityRanker, RankingWeights};
