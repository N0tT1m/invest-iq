//! Time Machine
//!
//! Interactive historical replay for learning from past market events.
//! Users can step through famous market scenarios, make trading decisions,
//! and compare their performance against AI recommendations.

pub mod replay;
pub mod scenarios;
pub mod scoring;

pub use replay::{
    BarData, DaySnapshot, ReplayEngine, ReplayState, SessionConfig, TimeMachineSession,
    TradeAction, UserDecision,
};
pub use scenarios::{Difficulty, Scenario, ScenarioLibrary};
pub use scoring::{DecisionScore, Leaderboard, LeaderboardEntry, ScoreCard, SessionScorer};
