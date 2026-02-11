use alpaca_broker::Position;
use anyhow::{bail, Result};

use crate::config::AgentConfig;

/// Portfolio-level risk guard (P4).
/// Enforces dynamic position limits, sector concentration, gross exposure, and daily loss limits.
pub struct PortfolioGuard {
    max_open_positions: usize,
    min_position_value: f64,
    max_sector_concentration: f64,
    max_gross_exposure: f64,
    daily_loss_halt_percent: f64,
}

impl PortfolioGuard {
    pub fn new(config: &AgentConfig) -> Self {
        Self {
            max_open_positions: config.max_open_positions,
            min_position_value: config.min_position_value,
            max_sector_concentration: config.max_sector_concentration,
            max_gross_exposure: config.max_gross_exposure,
            daily_loss_halt_percent: config.daily_loss_halt_percent,
        }
    }

    /// Compute the dynamic position limit based on portfolio size.
    /// Formula: min(floor(portfolio_value / min_position_value), max_open_positions)
    /// Ensures each position is meaningful relative to the account.
    fn dynamic_max_positions(&self, portfolio_value: f64) -> usize {
        if self.min_position_value <= 0.0 || portfolio_value <= 0.0 {
            return self.max_open_positions;
        }
        let computed = (portfolio_value / self.min_position_value) as usize;
        computed.clamp(1, self.max_open_positions)
    }

    /// Check whether a new trade is allowed given portfolio constraints.
    pub fn check_new_trade(
        &self,
        symbol: &str,
        action: &str,
        trade_amount: f64,
        positions: &[Position],
        portfolio_value: f64,
        daily_pl: f64,
    ) -> Result<()> {
        // Only gate new entries (BUY or SELL-to-open)
        if action != "BUY" && action != "SELL" {
            return Ok(());
        }

        // 1. Dynamic position limit (scales with portfolio size)
        let max_positions = self.dynamic_max_positions(portfolio_value);
        if positions.len() >= max_positions {
            bail!(
                "Portfolio guard: position limit {} reached (have {}, portfolio=${:.0}, min_per_position=${:.0})",
                max_positions,
                positions.len(),
                portfolio_value,
                self.min_position_value
            );
        }

        // 2. Gross exposure limit
        if portfolio_value > 0.0 {
            let gross_exposure: f64 = positions
                .iter()
                .filter_map(|p| p.market_value.parse::<f64>().ok())
                .map(|v| v.abs())
                .sum::<f64>()
                + trade_amount;
            let exposure_ratio = gross_exposure / portfolio_value;

            if exposure_ratio > self.max_gross_exposure {
                bail!(
                    "Portfolio guard: gross exposure {:.1}% exceeds limit {:.1}%",
                    exposure_ratio * 100.0,
                    self.max_gross_exposure * 100.0
                );
            }
        }

        // 3. Sector concentration
        let new_sector = symbol_to_sector(symbol);
        if portfolio_value > 0.0 {
            let mut sector_value = trade_amount;
            for pos in positions {
                if symbol_to_sector(&pos.symbol) == new_sector {
                    sector_value += pos.market_value.parse::<f64>().unwrap_or(0.0).abs();
                }
            }
            let sector_ratio = sector_value / portfolio_value;

            if sector_ratio > self.max_sector_concentration {
                // Allow if higher confidence
                bail!(
                    "Portfolio guard: sector '{}' concentration {:.1}% exceeds limit {:.1}%",
                    new_sector,
                    sector_ratio * 100.0,
                    self.max_sector_concentration * 100.0
                );
            }
        }

        // 4. Daily loss halt
        if portfolio_value > 0.0 {
            let daily_loss_pct = -(daily_pl / portfolio_value) * 100.0;
            if daily_loss_pct > self.daily_loss_halt_percent {
                bail!(
                    "Portfolio guard: daily loss {:.2}% exceeds halt threshold {:.1}%",
                    daily_loss_pct,
                    self.daily_loss_halt_percent
                );
            }
        }

        tracing::debug!(
            "Portfolio guard: {} {} approved (positions={}/{}, sector={})",
            action,
            symbol,
            positions.len(),
            max_positions,
            new_sector
        );

        Ok(())
    }
}

/// Map symbols to GICS-like sectors for concentration checks.
/// Covers the top ~100 most commonly traded US stocks.
fn symbol_to_sector(symbol: &str) -> &'static str {
    match symbol {
        // Technology
        "AAPL" | "MSFT" | "GOOGL" | "GOOG" | "META" | "NVDA" | "AMD" | "INTC" | "CRM"
        | "ORCL" | "ADBE" | "CSCO" | "AVGO" | "TXN" | "QCOM" | "NOW" | "IBM" | "AMAT"
        | "MU" | "LRCX" | "KLAC" | "SNPS" | "CDNS" | "MRVL" | "PANW" | "FTNT" | "CRWD" => {
            "Technology"
        }
        // Consumer Discretionary
        "AMZN" | "TSLA" | "HD" | "NKE" | "SBUX" | "TGT" | "LOW" | "MCD" | "BKNG" | "CMG"
        | "ABNB" | "LULU" | "ROST" | "TJX" | "ORLY" | "AZO" | "DPZ" => "Consumer Discretionary",
        // Communication
        "NFLX" | "DIS" | "CMCSA" | "T" | "VZ" | "TMUS" | "SPOT" | "ROKU" | "SNAP" | "PINS" => {
            "Communication"
        }
        // Financials
        "JPM" | "BAC" | "WFC" | "GS" | "MS" | "C" | "BLK" | "SCHW" | "AXP" | "V" | "MA"
        | "BRK.B" | "COF" | "USB" | "PNC" | "TFC" => "Financials",
        // Healthcare
        "JNJ" | "UNH" | "PFE" | "ABBV" | "MRK" | "LLY" | "TMO" | "ABT" | "DHR" | "BMY"
        | "AMGN" | "GILD" | "ISRG" | "MDT" | "SYK" | "CI" | "HUM" | "MRNA" | "BIIB" => {
            "Healthcare"
        }
        // Consumer Staples
        "PG" | "KO" | "PEP" | "WMT" | "COST" | "PM" | "MO" | "CL" | "KHC" | "MDLZ" | "STZ"
        | "KMB" => "Consumer Staples",
        // Energy
        "XOM" | "CVX" | "COP" | "SLB" | "EOG" | "MPC" | "PSX" | "VLO" | "OXY" | "HAL" => {
            "Energy"
        }
        // Industrials
        "BA" | "CAT" | "HON" | "UPS" | "RTX" | "GE" | "LMT" | "DE" | "MMM" | "UNP" | "FDX"
        | "WM" | "ETN" | "ITW" | "EMR" => "Industrials",
        // ETFs — treat as their own sector to avoid concentration issues
        "SPY" | "QQQ" | "DIA" | "IWM" | "VTI" | "VOO" | "XLF" | "XLK" | "XLE" | "XLV"
        | "XLI" | "XLU" | "XLP" | "XLY" | "XLB" | "XLRE" | "XLC" | "GLD" | "TLT" | "HYG"
        | "LQD" | "IEF" | "SHY" | "EEM" | "EFA" | "ARKK" => "ETFs",
        // Default
        _ => "Other",
    }
}
