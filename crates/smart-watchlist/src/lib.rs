//! Smart Watchlist Module
//!
//! AI-curated, personalized opportunity feed that learns user preferences.
//! Scans for opportunities and ranks them by personal relevance.

pub mod models;
pub mod opportunity_scanner;
pub mod preference_learner;
pub mod ranker;

pub use models::{
    EventType, InteractionType, Opportunity, OpportunitySignal, SymbolInteraction, UserPreference,
    WatchlistItem,
};
pub use opportunity_scanner::{OpportunityScanner, ScanConfig};
pub use preference_learner::PreferenceLearner;
pub use ranker::{OpportunityRanker, RankingWeights};
