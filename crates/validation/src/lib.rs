pub mod alpha_vantage;
pub mod backtesting;
pub mod comparison;
pub mod yahoo_finance;

pub use alpha_vantage::AlphaVantageClient;
pub use backtesting::{BacktestEngine, BacktestResult, TradeSignal};
pub use comparison::{ComparisonEngine, ComparisonResult, IndicatorDifference};
pub use yahoo_finance::YahooFinanceClient;
