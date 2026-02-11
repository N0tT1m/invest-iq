use analysis_core::{AnalysisError, AnalysisResult, AnalystConsensusData, Financials, FundamentalAnalyzer, SignalStrength};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

pub struct FundamentalAnalysisEngine;

impl FundamentalAnalysisEngine {
    pub fn new() -> Self {
        Self
    }

    fn calculate_pe_ratio(&self, price: f64, eps: f64) -> Option<f64> {
        if eps > 0.0 {
            Some(price / eps)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn calculate_pb_ratio(&self, price: f64, shares: f64, equity: f64) -> Option<f64> {
        if shares > 0.0 && equity > 0.0 {
            let book_value_per_share = equity / shares;
            if book_value_per_share > 0.0 {
                Some(price / book_value_per_share)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn calculate_debt_to_equity(&self, liabilities: f64, equity: f64) -> Option<f64> {
        if equity > 0.0 {
            Some(liabilities / equity)
        } else {
            None
        }
    }

    fn calculate_roe(&self, net_income: f64, equity: f64) -> Option<f64> {
        if equity > 0.0 {
            Some((net_income / equity) * 100.0)
        } else {
            None
        }
    }

    fn calculate_profit_margin(&self, net_income: f64, revenue: f64) -> Option<f64> {
        if revenue > 0.0 {
            Some((net_income / revenue) * 100.0)
        } else {
            None
        }
    }

    fn calculate_current_ratio(&self, assets: f64, liabilities: f64) -> Option<f64> {
        if liabilities > 0.0 {
            Some(assets / liabilities)
        } else {
            None
        }
    }

    fn calculate_gross_margin(&self, gross_profit: f64, revenue: f64) -> Option<f64> {
        if revenue > 0.0 {
            Some((gross_profit / revenue) * 100.0)
        } else {
            None
        }
    }

    fn calculate_operating_margin(&self, operating_income: f64, revenue: f64) -> Option<f64> {
        if revenue > 0.0 {
            Some((operating_income / revenue) * 100.0)
        } else {
            None
        }
    }

    /// Enhanced analysis that uses real current price and multi-quarter data
    pub fn analyze_enhanced(
        &self,
        symbol: &str,
        financials: &[Financials],
        current_price: Option<f64>,
        shares_outstanding: Option<f64>,
        risk_free_rate: Option<f64>,
    ) -> Result<AnalysisResult, AnalysisError> {
        if financials.is_empty() {
            return Err(AnalysisError::InsufficientData(
                "No financial data available".to_string(),
            ));
        }
        let price = current_price.unwrap_or(0.0);

        // Build TTM (trailing twelve months) by summing last 4 quarters for flow metrics.
        // Balance sheet items use the latest quarter (point-in-time).
        let ttm_quarters = financials.len().min(4);
        let ttm_slice = &financials[..ttm_quarters];

        // Helper: sum an Optional field across TTM quarters, returning None if all are None
        fn sum_ttm(quarters: &[Financials], accessor: fn(&Financials) -> Option<f64>) -> Option<f64> {
            let values: Vec<f64> = quarters.iter().filter_map(|f| accessor(f)).collect();
            if values.is_empty() { None } else { Some(values.iter().sum()) }
        }

        let ttm_revenue = sum_ttm(ttm_slice, |f| f.revenue);
        let ttm_gross_profit = sum_ttm(ttm_slice, |f| f.gross_profit);
        let ttm_operating_income = sum_ttm(ttm_slice, |f| f.operating_income);
        let ttm_net_income = sum_ttm(ttm_slice, |f| f.net_income);
        let ttm_eps = sum_ttm(ttm_slice, |f| f.eps);
        let ttm_ocf = sum_ttm(ttm_slice, |f| f.cash_flow_operating);
        let ttm_cfi = sum_ttm(ttm_slice, |f| f.cash_flow_investing);

        // Balance sheet: use latest quarter
        let latest = &financials[0];
        let bs_total_assets = latest.total_assets;
        let bs_total_liabilities = latest.total_liabilities;
        let bs_shareholders_equity = latest.shareholders_equity;

        let mut signals: Vec<(&str, i32, bool)> = Vec::new();
        let mut metrics_map = serde_json::Map::new();
        let mut data_fields_present: u32 = 0;
        let total_fields: u32 = 12;

        // Compute revenue growth YoY using TTM revenue vs prior-year TTM.
        // This is robust against missing quarters or gaps in Polygon data.
        let revenue_growth = if financials.len() >= 5 {
            let current_ttm: f64 = financials[..4].iter().filter_map(|f| f.revenue).sum();
            let prior_ttm: f64 = financials[4..financials.len().min(8)].iter().filter_map(|f| f.revenue).sum();
            let current_count = financials[..4].iter().filter(|f| f.revenue.is_some()).count();
            let prior_count = financials[4..financials.len().min(8)].iter().filter(|f| f.revenue.is_some()).count();
            if current_count >= 3 && prior_count >= 3 && prior_ttm > 0.0 {
                // Normalize if quarter counts differ
                let current_norm = current_ttm / current_count as f64 * 4.0;
                let prior_norm = prior_ttm / prior_count as f64 * 4.0;
                Some(((current_norm - prior_norm) / prior_norm) * 100.0)
            } else {
                None
            }
        } else {
            None
        };

        // P/E Ratio — uses TTM EPS with growth-adjusted thresholds
        if let (Some(eps), true) = (ttm_eps, price > 0.0) {
            if eps > 0.0 {
                let pe = price / eps;
                metrics_map.insert("pe_ratio".to_string(), json!(pe));
                data_fields_present += 1;

                // Adjust P/E thresholds based on growth rate
                let (low_pe_threshold, high_pe_threshold) = match revenue_growth {
                    Some(g) if g > 25.0 => (25.0, 60.0),  // High growth: tolerate higher P/E
                    Some(g) if g > 10.0 => (18.0, 40.0),  // Moderate growth
                    _ => (15.0, 30.0),                      // Low/no growth: standard thresholds
                };

                if pe < low_pe_threshold {
                    signals.push(("Low P/E Ratio", 3, true));
                } else if pe > high_pe_threshold {
                    signals.push(("High P/E Ratio", 2, false));
                }

                // PEG ratio if we have growth
                if let Some(growth) = revenue_growth {
                    if growth > 0.0 {
                        let peg = pe / growth;
                        metrics_map.insert("peg_ratio".to_string(), json!(peg));
                        if peg < 1.0 {
                            signals.push(("Attractive PEG Ratio", 2, true));
                        } else if peg > 2.5 {
                            signals.push(("Expensive PEG Ratio", 1, false));
                        }
                    }
                }
            }
        }

        // ROE (TTM net income / latest equity)
        if let (Some(net_income), Some(equity)) = (ttm_net_income, bs_shareholders_equity) {
            if let Some(roe) = self.calculate_roe(net_income, equity) {
                metrics_map.insert("roe".to_string(), json!(roe));
                data_fields_present += 1;
                if roe > 15.0 {
                    signals.push(("Strong ROE", 3, true));
                } else if roe < 5.0 {
                    signals.push(("Weak ROE", 2, false));
                }
            }
        }

        // Net Profit Margin (TTM)
        if let (Some(net_income), Some(revenue)) = (ttm_net_income, ttm_revenue) {
            if let Some(margin) = self.calculate_profit_margin(net_income, revenue) {
                metrics_map.insert("profit_margin".to_string(), json!(margin));
                data_fields_present += 1;
                if margin > 20.0 {
                    signals.push(("High Profit Margin", 3, true));
                } else if margin < 5.0 {
                    signals.push(("Low Profit Margin", 2, false));
                }
            }
        }

        // Gross Margin (TTM)
        if let (Some(gross_profit), Some(revenue)) = (ttm_gross_profit, ttm_revenue) {
            if let Some(gm) = self.calculate_gross_margin(gross_profit, revenue) {
                metrics_map.insert("gross_margin".to_string(), json!(gm));
                data_fields_present += 1;
                if gm > 50.0 {
                    signals.push(("High Gross Margin", 2, true));
                } else if gm < 20.0 {
                    signals.push(("Low Gross Margin", 2, false));
                }
            }
        }

        // Operating Margin (TTM)
        if let (Some(op_income), Some(revenue)) = (ttm_operating_income, ttm_revenue) {
            if let Some(om) = self.calculate_operating_margin(op_income, revenue) {
                metrics_map.insert("operating_margin".to_string(), json!(om));
                data_fields_present += 1;
                if om > 20.0 {
                    signals.push(("Strong Operating Margin", 2, true));
                } else if om < 5.0 {
                    signals.push(("Weak Operating Margin", 2, false));
                }
            }
        }

        // Debt-to-Equity (balance sheet)
        if let (Some(liabilities), Some(equity)) = (bs_total_liabilities, bs_shareholders_equity) {
            if let Some(d2e) = self.calculate_debt_to_equity(liabilities, equity) {
                metrics_map.insert("debt_to_equity".to_string(), json!(d2e));
                data_fields_present += 1;
                if d2e < 0.5 {
                    signals.push(("Low Debt", 2, true));
                } else if d2e > 2.0 {
                    signals.push(("High Debt", 3, false));
                }
            }
        }

        // Asset Coverage Ratio (balance sheet)
        if let (Some(assets), Some(liabilities)) = (bs_total_assets, bs_total_liabilities) {
            if let Some(acr) = self.calculate_current_ratio(assets, liabilities) {
                metrics_map.insert("asset_coverage_ratio".to_string(), json!(acr));
                data_fields_present += 1;
                if acr > 1.5 {
                    signals.push(("Strong Asset Coverage", 2, true));
                } else if acr < 1.0 {
                    signals.push(("Weak Asset Coverage", 2, false));
                }
            }
        }

        // Operating Cash Flow (TTM)
        if let Some(ocf) = ttm_ocf {
            data_fields_present += 1;
            if ocf > 0.0 {
                signals.push(("Positive Cash Flow", 2, true));
            } else {
                signals.push(("Negative Cash Flow", 3, false));
            }
            metrics_map.insert("operating_cash_flow".to_string(), json!(ocf));

            // Quality of Earnings: TTM OCF / TTM Net Income
            if let Some(net_income) = ttm_net_income {
                if net_income > 0.0 {
                    let qoe = ocf / net_income;
                    metrics_map.insert("quality_of_earnings".to_string(), json!(qoe));
                    if qoe > 1.0 {
                        signals.push(("High Earnings Quality (OCF>NI)", 2, true));
                    } else if qoe < 0.5 {
                        signals.push(("Low Earnings Quality", 2, false));
                    }
                }
            }

            // Free Cash Flow (TTM OCF + TTM Investing CF)
            // Note: uses total investing CF as capex proxy; includes acquisitions
            if let Some(cfi) = ttm_cfi {
                let fcf = ocf + cfi;
                metrics_map.insert("free_cash_flow".to_string(), json!(fcf));
                if fcf > 0.0 {
                    signals.push(("Positive Free Cash Flow", 2, true));
                } else if ocf > 0.0 {
                    // OCF positive but investing outflows exceed it — likely acquisitions/growth capex
                    signals.push(("Investment-Heavy (Negative FCF)", 1, false));
                } else {
                    signals.push(("Negative Free Cash Flow", 2, false));
                }
            }
        }

        // ROIC: TTM after-tax operating income / invested capital (balance sheet)
        if let (Some(op_income), Some(equity), Some(liabilities)) = (
            ttm_operating_income,
            bs_shareholders_equity,
            bs_total_liabilities,
        ) {
            let invested_capital = equity + liabilities;
            if invested_capital > 0.0 {
                let roic = (op_income * 0.79 / invested_capital) * 100.0; // 21% US corporate tax rate
                metrics_map.insert("roic".to_string(), json!(roic));
                if roic > 15.0 {
                    signals.push(("Strong ROIC", 2, true));
                } else if roic < 5.0 {
                    signals.push(("Weak ROIC", 2, false));
                }
            }
        }

        // --- DCF-Lite Intrinsic Value Estimate (uses TTM FCF) ---
        if let (Some(ocf), Some(cfi), Some(shares), true) = (
            ttm_ocf,
            ttm_cfi,
            shares_outstanding,
            price > 0.0,
        ) {
            if shares > 0.0 {
                let fcf = ocf + cfi;
                let fcf_per_share = fcf / shares;
                if fcf_per_share > 0.0 {
                    let growth_rate = revenue_growth
                        .map(|g| (g / 100.0).clamp(-0.05, 0.25))
                        .unwrap_or(0.03);
                    let rf = risk_free_rate.unwrap_or(0.045);
                    let equity_risk_premium = 0.055;
                    let discount_rate = (rf + equity_risk_premium).max(0.08);
                    let terminal_growth = 0.03;

                    let projected_fcf: f64 = (1_i32..=5)
                        .map(|i| {
                            fcf_per_share * (1.0_f64 + growth_rate).powi(i)
                                / (1.0_f64 + discount_rate).powi(i)
                        })
                        .sum();
                    let terminal_value = fcf_per_share
                        * (1.0_f64 + growth_rate).powi(5)
                        * (1.0_f64 + terminal_growth)
                        / (discount_rate - terminal_growth);
                    let terminal_pv = terminal_value / (1.0_f64 + discount_rate).powi(5);
                    let fair_value = projected_fcf + terminal_pv;

                    metrics_map.insert("fair_value_estimate".to_string(), json!(fair_value));
                    metrics_map.insert("price_to_fair_value".to_string(), json!(price / fair_value));

                    if price > fair_value * 1.5 {
                        signals.push(("Significantly Above Fair Value", 3, false));
                    } else if price > fair_value * 1.2 {
                        signals.push(("Trading Above Fair Value", 2, false));
                    } else if price < fair_value * 0.7 {
                        signals.push(("Significantly Below Fair Value", 3, true));
                    } else if price < fair_value * 0.9 {
                        signals.push(("Trading Below Fair Value", 2, true));
                    }
                }
            }
        }

        // --- Fundamental Value Score: quality + valuation ---
        // Composite quality score from existing metrics
        let has_strong_roe = metrics_map.get("roe").and_then(|v| v.as_f64()).map_or(false, |r| r > 15.0);
        let has_high_margins = metrics_map.get("profit_margin").and_then(|v| v.as_f64()).map_or(false, |m| m > 15.0);
        let has_positive_cf = ttm_ocf.map_or(false, |ocf| ocf > 0.0);
        let has_strong_roic = metrics_map.get("roic").and_then(|v| v.as_f64()).map_or(false, |r| r > 12.0);
        let has_rev_growth = revenue_growth.map_or(false, |g| g > 5.0);
        let has_low_debt = metrics_map.get("debt_to_equity").and_then(|v| v.as_f64()).map_or(false, |d| d < 1.5);

        let quality_score: u32 = [has_strong_roe, has_high_margins, has_positive_cf, has_strong_roic, has_rev_growth, has_low_debt]
            .iter()
            .filter(|&&v| v)
            .count() as u32;
        metrics_map.insert("quality_score".to_string(), json!(quality_score));

        if quality_score >= 4 {
            // High-quality company — check if undervalued relative to growth
            let high_pe_threshold = match revenue_growth {
                Some(g) if g > 25.0 => 60.0,
                Some(g) if g > 10.0 => 40.0,
                _ => 30.0,
            };
            if let Some(pe) = metrics_map.get("pe_ratio").and_then(|v| v.as_f64()) {
                if pe < high_pe_threshold * 0.75 {
                    signals.push(("Undervalued Quality Stock", 4, true));
                } else if pe < high_pe_threshold * 0.9 {
                    signals.push(("Fairly Valued Quality Stock", 2, true));
                }
            }
        }

        if quality_score >= 3 {
            if let Some(growth) = revenue_growth {
                if growth > 15.0 {
                    signals.push(("Growth At Reasonable Price", 3, true));
                }
            }
        }

        // Revenue Growth (already computed above for P/E adjustment)
        if let Some(growth) = revenue_growth {
            metrics_map.insert("revenue_growth".to_string(), json!(growth));
            data_fields_present += 1;
            if growth > 10.0 {
                signals.push(("Strong Revenue Growth", 3, true));
            } else if growth < -5.0 {
                signals.push(("Revenue Decline", 3, false));
            }
        }

        if let Some(revenue) = ttm_revenue {
            metrics_map.insert("revenue".to_string(), json!(revenue));
        }

        // Calculate overall signal
        let mut total_score = 0;
        let mut total_weight = 0;

        for (_, weight, bullish) in &signals {
            total_weight += weight;
            total_score += if *bullish { *weight } else { -weight };
        }

        let normalized_score = if total_weight > 0 {
            (total_score as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };

        let signal = SignalStrength::from_score(normalized_score as i32);

        // Dynamic confidence: signal_confidence * 0.6 + data_completeness * 0.4
        let signal_confidence = if signals.len() >= 5 {
            0.8
        } else if signals.len() >= 3 {
            0.6
        } else {
            0.4
        };
        let data_completeness = data_fields_present as f64 / total_fields as f64;
        let confidence = (signal_confidence * 0.6 + data_completeness * 0.4).min(0.95);

        let reason = signals
            .iter()
            .map(|(name, _, bullish)| {
                format!("{} {}", if *bullish { "+" } else { "-" }, name)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let metrics = json!(metrics_map);

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason: if reason.is_empty() { "Insufficient fundamental data".to_string() } else { reason },
            metrics,
        })
    }

    /// Enhanced analysis that incorporates analyst consensus data.
    /// Blends the original fundamental score (70%) with analyst consensus signals (30%).
    /// If no consensus data is available, falls through to analyze_enhanced unchanged.
    pub fn analyze_with_consensus(
        &self,
        symbol: &str,
        financials: &[Financials],
        current_price: Option<f64>,
        shares_outstanding: Option<f64>,
        consensus_data: &AnalystConsensusData,
        risk_free_rate: Option<f64>,
    ) -> Result<AnalysisResult, AnalysisError> {
        let mut result = self.analyze_enhanced(symbol, financials, current_price, shares_outstanding, risk_free_rate)?;

        // If no consensus data at all, return unchanged
        if consensus_data.consensus.is_none() && consensus_data.recent_ratings.is_empty() {
            return Ok(result);
        }

        let price = match current_price {
            Some(p) if p > 0.0 => p,
            _ => return Ok(result), // Can't compute upside without price
        };

        let mut consensus_signals: Vec<(&str, i32, bool)> = Vec::new();

        // Parse metrics from existing result to append analyst metrics
        let mut metrics_map: serde_json::Map<String, serde_json::Value> =
            if let Some(obj) = result.metrics.as_object() {
                obj.clone()
            } else {
                serde_json::Map::new()
            };

        // --- Consensus-level signals ---
        if let Some(consensus) = &consensus_data.consensus {
            // Store metrics
            if let Some(target) = consensus.consensus_price_target {
                metrics_map.insert("analyst_price_target".to_string(), serde_json::json!(target));
                let upside_pct = ((target - price) / price) * 100.0;
                metrics_map.insert("analyst_upside_pct".to_string(), serde_json::json!(upside_pct));

                // Price target signals
                if upside_pct > 20.0 {
                    consensus_signals.push(("Analyst Target >20% Upside", 3, true));
                } else if upside_pct > 10.0 {
                    consensus_signals.push(("Analyst Target >10% Upside", 2, true));
                } else if upside_pct < -20.0 {
                    consensus_signals.push(("Analyst Target >20% Downside", 3, false));
                } else if upside_pct < -10.0 {
                    consensus_signals.push(("Analyst Target >10% Downside", 2, false));
                }
            }

            if let Some(rating) = &consensus.consensus_rating {
                metrics_map.insert("analyst_consensus_rating".to_string(), serde_json::json!(rating));
                let r = rating.to_lowercase();
                if r.contains("strong buy") {
                    consensus_signals.push(("Consensus: Strong Buy", 2, true));
                } else if r.contains("buy") || r.contains("outperform") || r.contains("overweight") {
                    consensus_signals.push(("Consensus: Buy", 2, true));
                } else if r.contains("strong sell") || r.contains("underperform") {
                    consensus_signals.push(("Consensus: Strong Sell", 2, false));
                } else if r.contains("sell") || r.contains("underweight") {
                    consensus_signals.push(("Consensus: Sell", 2, false));
                }
            }

            if let Some(high) = consensus.high_price_target {
                metrics_map.insert("analyst_high_target".to_string(), serde_json::json!(high));
                // Price above ALL analyst targets = extreme overvaluation
                if price > high && high > 0.0 {
                    consensus_signals.push(("Price Above ALL Analyst Targets", 3, false));
                }
            }
            if let Some(low) = consensus.low_price_target {
                metrics_map.insert("analyst_low_target".to_string(), serde_json::json!(low));
                // Price below ALL analyst targets = clear undervaluation
                if price < low && low > 0.0 {
                    consensus_signals.push(("Price Below ALL Analyst Targets", 2, true));
                }
            }

            let count = consensus.contributors.or_else(|| {
                let b = consensus.buy_count.unwrap_or(0);
                let h = consensus.hold_count.unwrap_or(0);
                let s = consensus.sell_count.unwrap_or(0);
                let total = b + h + s;
                if total > 0 { Some(total) } else { None }
            });
            if let Some(c) = count {
                metrics_map.insert("analyst_count".to_string(), serde_json::json!(c));
            }
        }

        // --- Recent rating momentum (upgrades vs downgrades) ---
        if !consensus_data.recent_ratings.is_empty() {
            let mut upgrades = 0i32;
            let mut downgrades = 0i32;
            for rating in &consensus_data.recent_ratings {
                if let Some(action) = &rating.rating_action {
                    let a = action.to_lowercase();
                    if a.contains("upgrade") || a.contains("initiated") || a.contains("reiterate") && rating.rating.as_ref().map_or(false, |r| {
                        let rl = r.to_lowercase();
                        rl.contains("buy") || rl.contains("outperform") || rl.contains("overweight")
                    }) {
                        upgrades += 1;
                    } else if a.contains("downgrade") {
                        downgrades += 1;
                    }
                }
            }
            metrics_map.insert("analyst_upgrades_recent".to_string(), serde_json::json!(upgrades));
            metrics_map.insert("analyst_downgrades_recent".to_string(), serde_json::json!(downgrades));

            let net = upgrades - downgrades;
            if net >= 3 {
                consensus_signals.push(("Strong Upgrade Momentum", 2, true));
            } else if net >= 1 {
                consensus_signals.push(("Upgrade Momentum", 1, true));
            } else if net <= -3 {
                consensus_signals.push(("Strong Downgrade Momentum", 2, false));
            } else if net <= -1 {
                consensus_signals.push(("Downgrade Momentum", 1, false));
            }
        }

        // If no consensus signals were generated, return original result with added metrics
        if consensus_signals.is_empty() {
            result.metrics = serde_json::Value::Object(metrics_map);
            return Ok(result);
        }

        // Calculate consensus score
        let mut consensus_total_score = 0i32;
        let mut consensus_total_weight = 0i32;
        for (_, weight, bullish) in &consensus_signals {
            consensus_total_weight += weight;
            consensus_total_score += if *bullish { *weight } else { -weight };
        }

        let consensus_normalized = if consensus_total_weight > 0 {
            (consensus_total_score as f64 / consensus_total_weight as f64) * 100.0
        } else {
            0.0
        };

        // Blend: 70% original + 30% consensus
        let original_score = result.signal.to_score() as f64;
        let blended_score = original_score * 0.70 + consensus_normalized * 0.30;
        let new_signal = SignalStrength::from_score(blended_score as i32);

        // Append consensus reasons to existing reason
        let consensus_reason = consensus_signals
            .iter()
            .map(|(name, _, bullish)| format!("{} {}", if *bullish { "+" } else { "-" }, name))
            .collect::<Vec<_>>()
            .join(", ");

        let combined_reason = if result.reason.is_empty() {
            consensus_reason
        } else {
            format!("{}, {}", result.reason, consensus_reason)
        };

        result.signal = new_signal;
        result.reason = combined_reason;
        result.metrics = serde_json::Value::Object(metrics_map);

        Ok(result)
    }

    fn analyze_sync(
        &self,
        symbol: &str,
        financials: &Financials,
    ) -> Result<AnalysisResult, AnalysisError> {
        let mut signals = Vec::new();
        let mut metrics_map = serde_json::Map::new();

        // For this example, we'll use a hypothetical current price
        // In production, you'd fetch the current price
        let current_price = 100.0; // Placeholder

        // P/E Ratio Analysis
        if let (Some(eps), true) = (financials.eps, financials.eps.unwrap_or(0.0) > 0.0) {
            if let Some(pe) = self.calculate_pe_ratio(current_price, eps) {
                metrics_map.insert("pe_ratio".to_string(), json!(pe));

                // Typical P/E ratios: <15 undervalued, 15-25 fair, >25 overvalued
                if pe < 15.0 {
                    signals.push(("Low P/E Ratio", 3, true));
                } else if pe > 30.0 {
                    signals.push(("High P/E Ratio", 2, false));
                }
            }
        }

        // ROE Analysis
        if let (Some(net_income), Some(equity)) = (financials.net_income, financials.shareholders_equity) {
            if let Some(roe) = self.calculate_roe(net_income, equity) {
                metrics_map.insert("roe".to_string(), json!(roe));

                // ROE > 15% is generally considered good
                if roe > 15.0 {
                    signals.push(("Strong ROE", 3, true));
                } else if roe < 5.0 {
                    signals.push(("Weak ROE", 2, false));
                }
            }
        }

        // Profit Margin Analysis
        if let (Some(net_income), Some(revenue)) = (financials.net_income, financials.revenue) {
            if let Some(margin) = self.calculate_profit_margin(net_income, revenue) {
                metrics_map.insert("profit_margin".to_string(), json!(margin));

                if margin > 20.0 {
                    signals.push(("High Profit Margin", 3, true));
                } else if margin < 5.0 {
                    signals.push(("Low Profit Margin", 2, false));
                }
            }
        }

        // Debt-to-Equity Analysis
        if let (Some(liabilities), Some(equity)) = (financials.total_liabilities, financials.shareholders_equity) {
            if let Some(d2e) = self.calculate_debt_to_equity(liabilities, equity) {
                metrics_map.insert("debt_to_equity".to_string(), json!(d2e));

                // D/E < 0.5 is conservative, 0.5-1.5 moderate, >1.5 aggressive
                if d2e < 0.5 {
                    signals.push(("Low Debt", 2, true));
                } else if d2e > 2.0 {
                    signals.push(("High Debt", 3, false));
                }
            }
        }

        // Current Ratio (Liquidity)
        if let (Some(assets), Some(liabilities)) = (financials.total_assets, financials.total_liabilities) {
            if let Some(current_ratio) = self.calculate_current_ratio(assets, liabilities) {
                metrics_map.insert("current_ratio".to_string(), json!(current_ratio));

                // Current ratio > 1.5 is healthy
                if current_ratio > 1.5 {
                    signals.push(("Strong Liquidity", 2, true));
                } else if current_ratio < 1.0 {
                    signals.push(("Weak Liquidity", 2, false));
                }
            }
        }

        // Operating Cash Flow Analysis
        if let Some(ocf) = financials.cash_flow_operating {
            if ocf > 0.0 {
                signals.push(("Positive Cash Flow", 2, true));
            } else {
                signals.push(("Negative Cash Flow", 3, false));
            }
            metrics_map.insert("operating_cash_flow".to_string(), json!(ocf));
        }

        // Revenue Growth (would need historical data in production)
        if let Some(revenue) = financials.revenue {
            metrics_map.insert("revenue".to_string(), json!(revenue));
        }

        // Calculate overall signal
        let mut total_score = 0;
        let mut total_weight = 0;

        for (_, weight, bullish) in &signals {
            total_weight += weight;
            total_score += if *bullish { *weight } else { -weight };
        }

        let normalized_score = if total_weight > 0 {
            (total_score as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };

        let signal = SignalStrength::from_score(normalized_score as i32);
        let confidence = if signals.len() >= 5 {
            0.8
        } else if signals.len() >= 3 {
            0.6
        } else {
            0.4
        };

        let reason = signals
            .iter()
            .map(|(name, _, bullish)| {
                format!("{} {}", if *bullish { "+" } else { "-" }, name)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let metrics = json!(metrics_map);

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason: if reason.is_empty() { "Insufficient fundamental data".to_string() } else { reason },
            metrics,
        })
    }
}

#[async_trait]
impl FundamentalAnalyzer for FundamentalAnalysisEngine {
    async fn analyze(
        &self,
        symbol: &str,
        financials: &Financials,
    ) -> Result<AnalysisResult, AnalysisError> {
        self.analyze_sync(symbol, financials)
    }
}

impl Default for FundamentalAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
