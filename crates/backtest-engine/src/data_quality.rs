use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::prelude::*;

use crate::models::*;

/// Analyze historical data for quality issues and corporate events.
pub fn check_data_quality(
    historical_data: &HashMap<String, Vec<HistoricalBar>>,
) -> DataQualityReport {
    let mut total_bars = 0usize;
    let mut missing_dates = 0usize;
    let mut zero_volume_bars = 0usize;
    let mut price_spike_count = 0usize;
    let mut warnings: Vec<DataWarning> = Vec::new();
    let mut corporate_events: Vec<CorporateEvent> = Vec::new();
    let mut market_events: Vec<MarketEvent> = Vec::new();

    for (symbol, bars) in historical_data {
        total_bars += bars.len();

        for (i, bar) in bars.iter().enumerate() {
            // Zero volume check
            if bar.volume <= 0.0 {
                zero_volume_bars += 1;
                warnings.push(DataWarning {
                    date: bar.date.clone(),
                    symbol: symbol.clone(),
                    warning_type: "zero_volume".to_string(),
                    message: "Bar has zero or negative volume".to_string(),
                });
            }

            // Price consistency: high >= low, high >= open/close, low <= open/close
            let high = bar.high.to_f64().unwrap_or(0.0);
            let low = bar.low.to_f64().unwrap_or(0.0);
            let open = bar.open.to_f64().unwrap_or(0.0);
            let close = bar.close.to_f64().unwrap_or(0.0);

            if high < low || high < open || high < close || low > open || low > close {
                warnings.push(DataWarning {
                    date: bar.date.clone(),
                    symbol: symbol.clone(),
                    warning_type: "price_inconsistency".to_string(),
                    message: format!(
                        "OHLC inconsistent: O={:.2} H={:.2} L={:.2} C={:.2}",
                        open, high, low, close
                    ),
                });
            }

            // Price spike detection (>20% daily move)
            if i > 0 {
                let prev_close = bars[i - 1].close.to_f64().unwrap_or(1.0);
                if prev_close > 0.0 {
                    let pct_change = ((close - prev_close) / prev_close).abs();
                    if pct_change > 0.20 {
                        price_spike_count += 1;
                        // Classify: likely stock split if close ~ prev_close/2 or *2
                        let ratio = close / prev_close;
                        if (ratio - 0.5).abs() < 0.05 || (ratio - 2.0).abs() < 0.1 {
                            corporate_events.push(CorporateEvent {
                                date: bar.date.clone(),
                                symbol: symbol.clone(),
                                event_type: "possible_split".to_string(),
                                magnitude: ratio,
                            });
                        } else if pct_change > 0.50 {
                            market_events.push(MarketEvent {
                                date: bar.date.clone(),
                                event_type: "extreme_move".to_string(),
                                magnitude: pct_change * 100.0,
                            });
                        }
                    }
                }
            }

            // Gap detection (missing dates)
            if i > 0 {
                let prev_date = NaiveDate::parse_from_str(&bars[i - 1].date, "%Y-%m-%d").ok();
                let curr_date = NaiveDate::parse_from_str(&bar.date, "%Y-%m-%d").ok();
                if let (Some(pd), Some(cd)) = (prev_date, curr_date) {
                    let gap = (cd - pd).num_days();
                    // More than 4 calendar days = potential missing trading day
                    // (normal weekends are 3 days: Fri→Mon)
                    if gap > 4 {
                        missing_dates += (gap - 3) as usize; // rough estimate
                        warnings.push(DataWarning {
                            date: bar.date.clone(),
                            symbol: symbol.clone(),
                            warning_type: "date_gap".to_string(),
                            message: format!(
                                "{}-day gap between {} and {}",
                                gap,
                                bars[i - 1].date,
                                bar.date
                            ),
                        });
                    }
                }
            }
        }
    }

    // Cap warnings to 100 to avoid oversized reports
    warnings.truncate(100);

    DataQualityReport {
        total_bars,
        missing_dates,
        zero_volume_bars,
        price_spike_count,
        warnings,
        corporate_events,
        market_events,
    }
}
