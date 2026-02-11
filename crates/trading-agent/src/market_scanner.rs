use anyhow::Result;
use crate::config::AgentConfig;
use chrono::{Utc, Timelike, Datelike, Weekday};
use polygon_client::PolygonClient;
use multi_timeframe::Timeframe;
use market_regime_detector::MarketRegime;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MarketOpportunity {
    pub symbol: String,
    pub current_price: f64,
    pub volume: i64,
    pub price_change_percent: f64,
    pub volatility: f64,
    pub regime: MarketRegime,
    pub primary_timeframe: Timeframe,
    pub news_sentiment_score: f64,
    pub source: OpportunitySource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpportunitySource {
    Watchlist,
    MarketMover,
}

pub struct MarketScanner {
    config: AgentConfig,
    polygon_client: PolygonClient,
}

impl MarketScanner {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let polygon_client = PolygonClient::new(config.polygon_api_key.clone());

        Ok(Self {
            config,
            polygon_client,
        })
    }

    pub async fn scan(&self) -> Result<Vec<MarketOpportunity>> {
        if !self.is_market_open() {
            tracing::info!("Market closed — scanning for analysis only (no trades will execute)");
        }

        // Single API call: fetch snapshots for the entire US stock market
        let all_snapshots = self.polygon_client.get_all_snapshots().await?;
        tracing::info!("Fetched {} ticker snapshots from Polygon", all_snapshots.len());

        let min_volume = if self.is_extended_hours() {
            self.config.extended_hours_min_volume
        } else {
            self.config.regular_hours_min_volume
        };

        let watchlist_set: std::collections::HashSet<&str> =
            self.config.watchlist.iter().map(|s| s.as_str()).collect();

        let min_price_base = 5.0; // Absolute floor (even watchlist)
        let min_volatility = 0.005; // 0.5% intraday range

        let mut watchlist_opps = Vec::new();
        let mut mover_candidates = Vec::new();

        for snap in &all_snapshots {
            let symbol = &snap.ticker;

            // Skip non-standard tickers (warrants, units, preferred shares)
            if symbol.contains('.') || symbol.contains('-') || symbol.len() > 5 {
                continue;
            }

            let current_price = snap.last_trade
                .as_ref()
                .and_then(|t| t.p)
                .or_else(|| snap.day.as_ref().and_then(|d| d.c));

            let current_price = match current_price {
                Some(p) if p >= min_price_base => p,
                _ => continue,
            };

            let volume = snap.day.as_ref().and_then(|d| d.v).unwrap_or(0.0) as i64;
            let price_change_percent = snap.todays_change_perc.unwrap_or(0.0);

            let volatility = if let Some(day) = &snap.day {
                let high = day.h.unwrap_or(0.0);
                let low = day.l.unwrap_or(0.0);
                let close = day.c.unwrap_or(1.0);
                if close > 0.0 { (high - low) / close } else { 0.0 }
            } else {
                0.0
            };

            let is_watchlist = watchlist_set.contains(symbol.as_str());

            if is_watchlist {
                // Watchlist: minimal filters (just need basic data)
                if volume > 0 {
                    watchlist_opps.push(MarketOpportunity {
                        symbol: symbol.clone(),
                        current_price,
                        volume,
                        price_change_percent,
                        volatility,
                        regime: MarketRegime::Ranging,
                        primary_timeframe: Timeframe::Daily,
                        news_sentiment_score: 0.0,
                        source: OpportunitySource::Watchlist,
                    });
                }
            } else {
                // Non-watchlist: quality filters
                if volume <= min_volume {
                    continue;
                }
                if current_price < self.config.scanner_min_price {
                    continue;
                }
                if volatility < min_volatility {
                    continue;
                }

                // Dollar volume gate: price * volume must exceed threshold
                let dollar_volume = current_price * volume as f64;
                if dollar_volume < self.config.scanner_min_dollar_volume {
                    continue;
                }

                mover_candidates.push(MarketOpportunity {
                    symbol: symbol.clone(),
                    current_price,
                    volume,
                    price_change_percent,
                    volatility,
                    regime: MarketRegime::Ranging,
                    primary_timeframe: Timeframe::Daily,
                    news_sentiment_score: 0.0,
                    source: OpportunitySource::MarketMover,
                });
            }
        }

        // Sort movers by absolute price change — biggest movers first
        mover_candidates.sort_by(|a, b| {
            b.price_change_percent.abs()
                .partial_cmp(&a.price_change_percent.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N movers
        mover_candidates.truncate(self.config.scan_top_n_stocks);

        // Combine: watchlist first, then quality-filtered movers
        let mut final_opps = watchlist_opps;
        let mover_count = mover_candidates.len();
        final_opps.extend(mover_candidates);

        tracing::info!(
            "Market scan: {} opportunities ({} watchlist + {} quality movers)",
            final_opps.len(),
            final_opps.len() - mover_count,
            mover_count,
        );

        Ok(final_opps)
    }

    /// Check if the market is currently open for trading (regular or extended hours)
    pub fn is_market_open(&self) -> bool {
        let now = Utc::now().with_timezone(&chrono_tz::US::Eastern);

        // Skip weekends
        if now.weekday() == Weekday::Sat || now.weekday() == Weekday::Sun {
            return false;
        }

        let hour = now.hour();
        let minute = now.minute();
        let time_minutes = hour * 60 + minute;

        let regular_open = 9 * 60 + 30;
        let regular_close = 16 * 60;
        let premarket_open = 4 * 60;
        let afterhours_close = 20 * 60;

        if time_minutes >= regular_open && time_minutes < regular_close {
            return true;
        }

        if self.config.enable_extended_hours {
            if time_minutes >= premarket_open && time_minutes < regular_open {
                return true;
            }
            if time_minutes >= regular_close && time_minutes < afterhours_close {
                return true;
            }
        }

        false
    }

    /// Check if we're in extended hours
    fn is_extended_hours(&self) -> bool {
        let now = Utc::now().with_timezone(&chrono_tz::US::Eastern);
        let hour = now.hour();
        let minute = now.minute();
        let time_minutes = hour * 60 + minute;

        let regular_open = 9 * 60 + 30;
        let regular_close = 16 * 60;

        time_minutes < regular_open || time_minutes >= regular_close
    }
}
