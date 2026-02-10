pub mod alpha_vantage;
pub mod yahoo_finance;
pub mod comparison;
pub mod backtesting;

pub use alpha_vantage::AlphaVantageClient;
pub use yahoo_finance::YahooFinanceClient;
pub use comparison::{ComparisonEngine, ComparisonResult, IndicatorDifference};
pub use backtesting::{BacktestEngine, BacktestResult, TradeSignal};
