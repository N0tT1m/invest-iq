//! Historical Replay Engine
//!
//! Manages time machine sessions, stepping through historical data
//! while hiding future information from the user.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for starting a new session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Optional scenario ID (if using pre-built scenario)
    pub scenario_id: Option<String>,
    /// Symbol to trade
    pub symbol: String,
    /// Start date for replay
    pub start_date: NaiveDate,
    /// End date for replay
    pub end_date: NaiveDate,
    /// Starting portfolio value
    pub starting_capital: f64,
    /// Optional user ID
    pub user_id: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            scenario_id: None,
            symbol: "SPY".to_string(),
            start_date: NaiveDate::from_ymd_opt(2020, 3, 2).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2020, 3, 23).unwrap(),
            starting_capital: 10000.0,
            user_id: None,
        }
    }
}

/// Current state of a replay session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayState {
    /// Session is active, awaiting user decisions
    Active,
    /// Session completed successfully
    Completed,
    /// Session was abandoned
    Abandoned,
    /// Session paused
    Paused,
}

impl std::fmt::Display for ReplayState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayState::Active => write!(f, "active"),
            ReplayState::Completed => write!(f, "completed"),
            ReplayState::Abandoned => write!(f, "abandoned"),
            ReplayState::Paused => write!(f, "paused"),
        }
    }
}

/// A user's trading decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDecision {
    /// Date of decision
    pub decision_date: NaiveDate,
    /// Action taken
    pub action: TradeAction,
    /// Number of shares (for buy/sell)
    pub shares: Option<i32>,
    /// Price at decision time
    pub price: f64,
    /// AI's recommendation at this point
    pub ai_recommendation: TradeAction,
    /// Actual next-day return (revealed after decision)
    pub actual_return: Option<f64>,
    /// Reasoning for the decision
    pub reason: Option<String>,
}

/// Possible trading actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeAction {
    Buy,
    Sell,
    Hold,
}

impl std::fmt::Display for TradeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeAction::Buy => write!(f, "buy"),
            TradeAction::Sell => write!(f, "sell"),
            TradeAction::Hold => write!(f, "hold"),
        }
    }
}

impl std::str::FromStr for TradeAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "buy" => Ok(TradeAction::Buy),
            "sell" => Ok(TradeAction::Sell),
            "hold" => Ok(TradeAction::Hold),
            _ => Err(anyhow::anyhow!("Invalid trade action: {}", s)),
        }
    }
}

/// A snapshot of market data for a single day
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySnapshot {
    /// Date of this snapshot
    pub date: NaiveDate,
    /// Opening price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Volume
    pub volume: u64,
    /// Previous bars visible to user (historical context)
    pub visible_history: Vec<BarData>,
    /// AI recommendation for this day
    pub ai_recommendation: TradeAction,
    /// AI confidence level (0-1)
    pub ai_confidence: f64,
    /// News headlines for this day (without spoilers)
    pub headlines: Vec<String>,
    /// Technical indicators
    pub indicators: TechnicalIndicators,
    /// Is this the last day of the scenario?
    pub is_final_day: bool,
}

/// OHLCV bar data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarData {
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Technical indicators for decision support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub rsi_14: f64,
    pub sma_20: f64,
    pub sma_50: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub bollinger_upper: f64,
    pub bollinger_lower: f64,
    pub atr_14: f64,
}

impl Default for TechnicalIndicators {
    fn default() -> Self {
        Self {
            rsi_14: 50.0,
            sma_20: 0.0,
            sma_50: 0.0,
            macd: 0.0,
            macd_signal: 0.0,
            bollinger_upper: 0.0,
            bollinger_lower: 0.0,
            atr_14: 0.0,
        }
    }
}

/// A complete time machine session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMachineSession {
    /// Unique session ID
    pub id: String,
    /// User ID (optional)
    pub user_id: Option<String>,
    /// Scenario ID (if using pre-built scenario)
    pub scenario_id: Option<String>,
    /// Symbol being traded
    pub symbol: String,
    /// Start date
    pub start_date: NaiveDate,
    /// End date
    pub end_date: NaiveDate,
    /// Current date in the replay
    pub current_date: NaiveDate,
    /// Starting capital
    pub starting_capital: f64,
    /// Current portfolio value
    pub portfolio_value: f64,
    /// Current cash position
    pub cash: f64,
    /// Current shares held
    pub shares_held: i32,
    /// Average cost basis
    pub cost_basis: f64,
    /// All decisions made
    pub decisions: Vec<UserDecision>,
    /// Session state
    pub status: ReplayState,
    /// Created timestamp
    pub created_at: chrono::DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: chrono::DateTime<Utc>,
}

impl TimeMachineSession {
    /// Create a new session from config
    pub fn new(config: SessionConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: config.user_id,
            scenario_id: config.scenario_id,
            symbol: config.symbol,
            start_date: config.start_date,
            end_date: config.end_date,
            current_date: config.start_date,
            starting_capital: config.starting_capital,
            portfolio_value: config.starting_capital,
            cash: config.starting_capital,
            shares_held: 0,
            cost_basis: 0.0,
            decisions: Vec::new(),
            status: ReplayState::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if session is complete
    pub fn is_complete(&self) -> bool {
        self.current_date >= self.end_date || self.status == ReplayState::Completed
    }

    /// Get total return percentage
    pub fn total_return_pct(&self) -> f64 {
        ((self.portfolio_value - self.starting_capital) / self.starting_capital) * 100.0
    }

    /// Get number of days completed
    pub fn days_completed(&self) -> i64 {
        (self.current_date - self.start_date).num_days()
    }

    /// Get total days in scenario
    pub fn total_days(&self) -> i64 {
        (self.end_date - self.start_date).num_days()
    }

    /// Get progress percentage
    pub fn progress_pct(&self) -> f64 {
        let total = self.total_days() as f64;
        if total <= 0.0 {
            return 100.0;
        }
        (self.days_completed() as f64 / total) * 100.0
    }
}

/// Engine for running time machine replays
pub struct ReplayEngine {
    /// Historical price data cache
    price_cache: std::collections::HashMap<String, Vec<BarData>>,
}

impl ReplayEngine {
    /// Create a new replay engine
    pub fn new() -> Self {
        Self {
            price_cache: std::collections::HashMap::new(),
        }
    }

    /// Start a new session
    pub fn start_session(&self, config: SessionConfig) -> TimeMachineSession {
        TimeMachineSession::new(config)
    }

    /// Get the current day snapshot for a session
    pub fn get_current_snapshot(
        &self,
        session: &TimeMachineSession,
        price_data: &[BarData],
    ) -> Option<DaySnapshot> {
        // Find current day in price data
        let current_idx = price_data
            .iter()
            .position(|b| b.date == session.current_date)?;

        let current_bar = &price_data[current_idx];

        // Get visible history (all bars up to and including current)
        let visible_history: Vec<BarData> = price_data[..=current_idx].to_vec();

        // Calculate technical indicators
        let indicators = self.calculate_indicators(&visible_history);

        // Generate AI recommendation
        let (ai_recommendation, ai_confidence) = self.generate_ai_recommendation(&indicators, current_bar);

        // Check if this is the final day
        let is_final_day = session.current_date >= session.end_date
            || current_idx >= price_data.len() - 1;

        Some(DaySnapshot {
            date: session.current_date,
            open: current_bar.open,
            high: current_bar.high,
            low: current_bar.low,
            close: current_bar.close,
            volume: current_bar.volume,
            visible_history,
            ai_recommendation,
            ai_confidence,
            headlines: Vec::new(), // Would come from news data
            indicators,
            is_final_day,
        })
    }

    /// Advance session to next day after user decision
    pub fn advance_session(
        &self,
        session: &mut TimeMachineSession,
        decision: UserDecision,
        price_data: &[BarData],
    ) -> anyhow::Result<Option<DaySnapshot>> {
        // Apply the decision to portfolio
        self.apply_decision(session, &decision)?;

        // Record the decision
        session.decisions.push(decision);

        // Find next trading day
        let current_idx = price_data
            .iter()
            .position(|b| b.date == session.current_date)
            .ok_or_else(|| anyhow::anyhow!("Current date not found in price data"))?;

        if current_idx >= price_data.len() - 1 {
            // No more days, complete the session
            session.status = ReplayState::Completed;
            session.updated_at = Utc::now();
            return Ok(None);
        }

        // Advance to next day
        let next_bar = &price_data[current_idx + 1];
        session.current_date = next_bar.date;

        // Update portfolio value based on new price
        self.update_portfolio_value(session, next_bar.close);

        // Check if we've reached the end
        if session.current_date >= session.end_date {
            session.status = ReplayState::Completed;
        }

        session.updated_at = Utc::now();

        // Return next day snapshot
        Ok(self.get_current_snapshot(session, price_data))
    }

    /// Apply a trading decision to the portfolio
    fn apply_decision(
        &self,
        session: &mut TimeMachineSession,
        decision: &UserDecision,
    ) -> anyhow::Result<()> {
        match decision.action {
            TradeAction::Buy => {
                let shares = decision.shares.unwrap_or(0);
                let cost = decision.price * shares as f64;

                if cost > session.cash {
                    return Err(anyhow::anyhow!("Insufficient cash for purchase"));
                }

                // Update cost basis (weighted average)
                let total_shares = session.shares_held + shares;
                if total_shares > 0 {
                    session.cost_basis = (session.cost_basis * session.shares_held as f64
                        + decision.price * shares as f64)
                        / total_shares as f64;
                }

                session.cash -= cost;
                session.shares_held += shares;
            }
            TradeAction::Sell => {
                let shares = decision.shares.unwrap_or(0);
                if shares > session.shares_held {
                    return Err(anyhow::anyhow!("Insufficient shares to sell"));
                }

                let proceeds = decision.price * shares as f64;
                session.cash += proceeds;
                session.shares_held -= shares;

                if session.shares_held == 0 {
                    session.cost_basis = 0.0;
                }
            }
            TradeAction::Hold => {
                // No action needed
            }
        }

        // Update portfolio value
        self.update_portfolio_value(session, decision.price);

        Ok(())
    }

    /// Update portfolio value based on current price
    fn update_portfolio_value(&self, session: &mut TimeMachineSession, current_price: f64) {
        session.portfolio_value = session.cash + (session.shares_held as f64 * current_price);
    }

    /// Calculate technical indicators from price history
    fn calculate_indicators(&self, history: &[BarData]) -> TechnicalIndicators {
        if history.is_empty() {
            return TechnicalIndicators::default();
        }

        let closes: Vec<f64> = history.iter().map(|b| b.close).collect();
        let n = closes.len();

        // RSI 14
        let rsi = if n >= 14 {
            self.calculate_rsi(&closes, 14)
        } else {
            50.0
        };

        // SMAs
        let sma_20 = if n >= 20 {
            closes[n - 20..].iter().sum::<f64>() / 20.0
        } else {
            closes.iter().sum::<f64>() / n as f64
        };

        let sma_50 = if n >= 50 {
            closes[n - 50..].iter().sum::<f64>() / 50.0
        } else {
            closes.iter().sum::<f64>() / n as f64
        };

        // Simple MACD approximation
        let ema_12 = self.calculate_ema(&closes, 12);
        let ema_26 = self.calculate_ema(&closes, 26);
        let macd = ema_12 - ema_26;
        let macd_signal = macd * 0.9; // Simplified

        // Bollinger Bands
        let std_dev = self.calculate_std_dev(&closes, 20);
        let bollinger_upper = sma_20 + 2.0 * std_dev;
        let bollinger_lower = sma_20 - 2.0 * std_dev;

        // ATR
        let atr = self.calculate_atr(history, 14);

        TechnicalIndicators {
            rsi_14: rsi,
            sma_20,
            sma_50,
            macd,
            macd_signal,
            bollinger_upper,
            bollinger_lower,
            atr_14: atr,
        }
    }

    fn calculate_rsi(&self, closes: &[f64], period: usize) -> f64 {
        if closes.len() < period + 1 {
            return 50.0;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (closes.len() - period)..closes.len() {
            let change = closes[i] - closes[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    fn calculate_ema(&self, data: &[f64], period: usize) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let k = 2.0 / (period as f64 + 1.0);
        let mut ema = data[0];

        for &value in &data[1..] {
            ema = value * k + ema * (1.0 - k);
        }

        ema
    }

    fn calculate_std_dev(&self, data: &[f64], period: usize) -> f64 {
        let n = data.len().min(period);
        if n < 2 {
            return 0.0;
        }

        let slice = &data[data.len() - n..];
        let mean = slice.iter().sum::<f64>() / n as f64;
        let variance = slice.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        variance.sqrt()
    }

    fn calculate_atr(&self, history: &[BarData], period: usize) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len().min(period);
        let mut tr_sum = 0.0;

        for i in (history.len() - n)..history.len() {
            let high = history[i].high;
            let low = history[i].low;
            let prev_close = if i > 0 { history[i - 1].close } else { history[i].open };

            let tr = (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());
            tr_sum += tr;
        }

        tr_sum / n as f64
    }

    /// Generate AI recommendation based on indicators
    fn generate_ai_recommendation(
        &self,
        indicators: &TechnicalIndicators,
        current_bar: &BarData,
    ) -> (TradeAction, f64) {
        let mut score = 0.0;
        let mut factors = 0;

        // RSI signals
        if indicators.rsi_14 < 30.0 {
            score += 1.0; // Oversold, bullish
            factors += 1;
        } else if indicators.rsi_14 > 70.0 {
            score -= 1.0; // Overbought, bearish
            factors += 1;
        }

        // Moving average signals
        if current_bar.close > indicators.sma_20 && indicators.sma_20 > indicators.sma_50 {
            score += 1.0; // Uptrend
            factors += 1;
        } else if current_bar.close < indicators.sma_20 && indicators.sma_20 < indicators.sma_50 {
            score -= 1.0; // Downtrend
            factors += 1;
        }

        // MACD signal
        if indicators.macd > indicators.macd_signal {
            score += 0.5;
            factors += 1;
        } else {
            score -= 0.5;
            factors += 1;
        }

        // Bollinger band signals
        if current_bar.close < indicators.bollinger_lower {
            score += 0.5; // Oversold
            factors += 1;
        } else if current_bar.close > indicators.bollinger_upper {
            score -= 0.5; // Overbought
            factors += 1;
        }

        let avg_score = if factors > 0 {
            score / factors as f64
        } else {
            0.0
        };

        let confidence = (avg_score.abs() * 0.5 + 0.3).min(0.9);

        let action = if avg_score > 0.3 {
            TradeAction::Buy
        } else if avg_score < -0.3 {
            TradeAction::Sell
        } else {
            TradeAction::Hold
        };

        (action, confidence)
    }

    /// Load price data for a symbol/date range
    pub fn load_price_data(
        &mut self,
        _symbol: &str,
        _start_date: NaiveDate,
        _end_date: NaiveDate,
    ) -> Vec<BarData> {
        // This would fetch from database/API
        // For now, return empty - actual implementation would query polygon-client
        Vec::new()
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = TimeMachineSession::new(config);

        assert_eq!(session.portfolio_value, 10000.0);
        assert_eq!(session.cash, 10000.0);
        assert_eq!(session.shares_held, 0);
        assert_eq!(session.status, ReplayState::Active);
    }

    #[test]
    fn test_trade_action_parse() {
        assert_eq!("buy".parse::<TradeAction>().unwrap(), TradeAction::Buy);
        assert_eq!("SELL".parse::<TradeAction>().unwrap(), TradeAction::Sell);
        assert_eq!("Hold".parse::<TradeAction>().unwrap(), TradeAction::Hold);
    }

    #[test]
    fn test_total_return_calculation() {
        let mut session = TimeMachineSession::new(SessionConfig::default());
        session.portfolio_value = 11000.0;

        assert!((session.total_return_pct() - 10.0).abs() < 0.01);
    }
}
