pub mod advanced_risk;
pub mod circuit_breaker;
pub mod commission;
pub mod data_quality;
pub mod db;
pub mod engine;
pub mod extended_metrics;
pub mod factor_attribution;
pub mod margin;
pub mod market_impact;
pub mod models;
pub mod monte_carlo;
pub mod order_types;
pub mod overfitting;
pub mod regime_risk;
pub mod short_selling;
pub mod statistical;
pub mod tear_sheet;
pub mod timeframe_agg;
pub mod trade_analysis;
pub mod trailing_stop;
pub mod walk_forward_opt;

#[cfg(test)]
mod tests;

pub use db::BacktestDb;
pub use engine::{BacktestEngine, WalkForwardRunner};
pub use models::*;
pub use monte_carlo::run_monte_carlo;
