pub mod models;
pub mod engine;
pub mod db;
pub mod monte_carlo;
pub mod commission;
pub mod data_quality;
pub mod short_selling;
pub mod margin;
pub mod order_types;
pub mod trailing_stop;
pub mod circuit_breaker;
pub mod regime_risk;
pub mod timeframe_agg;
pub mod extended_metrics;
pub mod factor_attribution;
pub mod statistical;
pub mod tear_sheet;
pub mod walk_forward_opt;
pub mod market_impact;
pub mod advanced_risk;
pub mod trade_analysis;
pub mod overfitting;

#[cfg(test)]
mod tests;

pub use models::*;
pub use engine::{BacktestEngine, WalkForwardRunner};
pub use db::BacktestDb;
pub use monte_carlo::run_monte_carlo;
