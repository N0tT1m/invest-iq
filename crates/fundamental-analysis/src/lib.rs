use analysis_core::{adaptive, AnalysisError, AnalysisResult, AnalystConsensusData, Financials, FundamentalAnalyzer, SignalStrength};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

fn classify_sector(sic_desc: Option<&str>) -> &'static str {
    let desc = match sic_desc {
        Some(d) => d.to_lowercase(),
        None => return "unknown",
    };
    if desc.contains("software") || desc.contains("semiconductor") || desc.contains("computer")
        || desc.contains("electronic") || desc.contains("data processing") || desc.contains("technology") {
        "technology"
    } else if desc.contains("pharma") || desc.contains("biological") || desc.contains("medical")
        || desc.contains("health") || desc.contains("biotech") {
        "healthcare"
    } else if desc.contains("bank") || desc.contains("insurance") || desc.contains("credit")
        || desc.contains("securities") || desc.contains("financial") || desc.contains("invest") {
        "financial"
    } else if desc.contains("electric") && (desc.contains("utility") || desc.contains("service")) || desc.contains("natural gas distribution") || desc.contains("water supply") {
        "utilities"
    } else if desc.contains("petroleum") || desc.contains("crude") || desc.contains("oil")
        || desc.contains("natural gas") || desc.contains("mining") || desc.contains("coal") {
        "energy"
    } else if desc.contains("food") || desc.contains("beverage") || desc.contains("tobacco")
        || desc.contains("household") || desc.contains("soap") || desc.contains("retail") {
        "consumer_staples"
    } else if desc.contains("auto") || desc.contains("aircraft") || desc.contains("industrial")
        || desc.contains("machinery") || desc.contains("manufacturing") || desc.contains("railroad") {
        "industrial"
    } else if desc.contains("real estate") || desc.contains("reit") {
        "real_estate"
    } else if desc.contains("telecom") || desc.contains("communication") || desc.contains("broadcast") {
        "telecom"
    } else {
        "unknown"
    }
}

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
        sic_description: Option<&str>,
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
        let ttm_cff = sum_ttm(ttm_slice, |f| f.cash_flow_financing);

        // Balance sheet: use latest quarter
        let latest = &financials[0];
        let bs_total_assets = latest.total_assets;
        let bs_total_liabilities = latest.total_liabilities;
        let bs_shareholders_equity = latest.shareholders_equity;

        let mut signals: Vec<(&str, i32, bool)> = Vec::new();
        let mut metrics_map = serde_json::Map::new();
        let mut data_fields_present: u32 = 0;
        let total_fields: u32 = 18; // increased for new metrics

        let sector = classify_sector(sic_description);
        metrics_map.insert("sector".to_string(), json!(sector));

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

        // P/E Ratio — uses growth-implied fair P/E
        if let (Some(eps), true) = (ttm_eps, price > 0.0) {
            if eps > 0.0 {
                let pe = price / eps;
                metrics_map.insert("pe_ratio".to_string(), json!(pe));
                data_fields_present += 1;

                // Growth-implied fair P/E using dividend discount model logic
                let growth_rate = revenue_growth.unwrap_or(0.0) / 100.0;
                let rf = risk_free_rate.unwrap_or(0.045);
                let equity_risk_premium = 0.055;
                let implied_pe = (1.0 / (rf + equity_risk_premium - growth_rate.min(0.08)))
                    .max(5.0)
                    .min(80.0);
                let pe_z = (pe - implied_pe) / implied_pe.max(1.0);

                metrics_map.insert("implied_pe".to_string(), json!(implied_pe));
                metrics_map.insert("pe_z_score".to_string(), json!(pe_z));

                if pe_z < -0.3 {
                    let weight = adaptive::z_score_to_weight(pe_z.abs());
                    signals.push(("Low P/E (vs Growth-Implied)", weight, true));
                } else if pe_z > 0.5 {
                    let weight = adaptive::z_score_to_weight(pe_z);
                    signals.push(("High P/E (vs Growth-Implied)", weight, false));
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

        // ROE (TTM net income / latest equity) — adaptive thresholds based on historical z-score
        if let (Some(net_income), Some(equity)) = (ttm_net_income, bs_shareholders_equity) {
            if let Some(roe) = self.calculate_roe(net_income, equity) {
                metrics_map.insert("roe".to_string(), json!(roe));
                data_fields_present += 1;

                // Compute historical ROE from available quarters
                let roe_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(ni), Some(eq)) = (f.net_income, f.shareholders_equity) {
                            if eq > 0.0 {
                                Some((ni / eq) * 100.0)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if roe_history.len() >= 3 {
                    // Use historical z-score
                    let roe_z = adaptive::z_score_of(roe, &roe_history);
                    metrics_map.insert("roe_z_score".to_string(), json!(roe_z));

                    if roe_z > 1.0 {
                        let weight = adaptive::z_score_to_weight(roe_z);
                        signals.push(("Strong ROE (vs History)", weight, true));
                    } else if roe_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(roe_z.abs());
                        signals.push(("Weak ROE (vs History)", weight, false));
                    }
                } else {
                    // Fall back to absolute thresholds if insufficient history
                    if roe > 12.0 {
                        signals.push(("Strong ROE", 3, true));
                    } else if roe < 3.0 {
                        signals.push(("Weak ROE", 2, false));
                    }
                }
            }
        }

        // Net Profit Margin (TTM) — adaptive based on historical z-score
        if let (Some(net_income), Some(revenue)) = (ttm_net_income, ttm_revenue) {
            if let Some(margin) = self.calculate_profit_margin(net_income, revenue) {
                metrics_map.insert("profit_margin".to_string(), json!(margin));
                data_fields_present += 1;

                // Compute historical profit margins
                let margin_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(ni), Some(rev)) = (f.net_income, f.revenue) {
                            if rev > 0.0 {
                                Some((ni / rev) * 100.0)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if margin_history.len() >= 3 {
                    let margin_z = adaptive::z_score_of(margin, &margin_history);
                    metrics_map.insert("profit_margin_z_score".to_string(), json!(margin_z));

                    if margin_z > 1.0 {
                        let weight = adaptive::z_score_to_weight(margin_z);
                        signals.push(("High Profit Margin (vs History)", weight, true));
                    } else if margin_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(margin_z.abs());
                        signals.push(("Low Profit Margin (vs History)", weight, false));
                    }
                } else {
                    // Fall back to absolute
                    if margin > 20.0 {
                        signals.push(("High Profit Margin", 3, true));
                    } else if margin < 5.0 {
                        signals.push(("Low Profit Margin", 2, false));
                    }
                }
            }
        }

        // Gross Margin (TTM) — adaptive based on historical z-score
        if let (Some(gross_profit), Some(revenue)) = (ttm_gross_profit, ttm_revenue) {
            if let Some(gm) = self.calculate_gross_margin(gross_profit, revenue) {
                metrics_map.insert("gross_margin".to_string(), json!(gm));
                data_fields_present += 1;

                // Compute historical gross margins
                let gm_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(gp), Some(rev)) = (f.gross_profit, f.revenue) {
                            if rev > 0.0 {
                                Some((gp / rev) * 100.0)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if gm_history.len() >= 3 {
                    let gm_z = adaptive::z_score_of(gm, &gm_history);
                    metrics_map.insert("gross_margin_z_score".to_string(), json!(gm_z));

                    if gm_z > 1.0 {
                        let weight = adaptive::z_score_to_weight(gm_z);
                        signals.push(("High Gross Margin (vs History)", weight, true));
                    } else if gm_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(gm_z.abs());
                        signals.push(("Low Gross Margin (vs History)", weight, false));
                    }
                } else {
                    // Fall back to absolute
                    if gm > 50.0 {
                        signals.push(("High Gross Margin", 2, true));
                    } else if gm < 20.0 {
                        signals.push(("Low Gross Margin", 2, false));
                    }
                }
            }
        }

        // Operating Margin (TTM) — adaptive based on historical z-score
        if let (Some(op_income), Some(revenue)) = (ttm_operating_income, ttm_revenue) {
            if let Some(om) = self.calculate_operating_margin(op_income, revenue) {
                metrics_map.insert("operating_margin".to_string(), json!(om));
                data_fields_present += 1;

                // Compute historical operating margins
                let om_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(oi), Some(rev)) = (f.operating_income, f.revenue) {
                            if rev > 0.0 {
                                Some((oi / rev) * 100.0)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if om_history.len() >= 3 {
                    let om_z = adaptive::z_score_of(om, &om_history);
                    metrics_map.insert("operating_margin_z_score".to_string(), json!(om_z));

                    if om_z > 1.0 {
                        let weight = adaptive::z_score_to_weight(om_z);
                        signals.push(("Strong Operating Margin (vs History)", weight, true));
                    } else if om_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(om_z.abs());
                        signals.push(("Weak Operating Margin (vs History)", weight, false));
                    }
                } else {
                    // Fall back to absolute
                    if om > 20.0 {
                        signals.push(("Strong Operating Margin", 2, true));
                    } else if om < 5.0 {
                        signals.push(("Weak Operating Margin", 2, false));
                    }
                }
            }
        }

        // Debt-to-Equity (balance sheet) — adaptive based on historical z-score
        if let (Some(liabilities), Some(equity)) = (bs_total_liabilities, bs_shareholders_equity) {
            if let Some(d2e) = self.calculate_debt_to_equity(liabilities, equity) {
                metrics_map.insert("debt_to_equity".to_string(), json!(d2e));
                data_fields_present += 1;

                // Compute historical D/E from available quarters
                let de_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(tl), Some(eq)) = (f.total_liabilities, f.shareholders_equity) {
                            if eq > 0.0 {
                                Some(tl / eq)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if de_history.len() >= 2 {
                    let de_z = adaptive::z_score_of(d2e, &de_history);
                    metrics_map.insert("debt_to_equity_z_score".to_string(), json!(de_z));

                    if de_z > 1.5 {
                        let weight = adaptive::z_score_to_weight(de_z);
                        signals.push(("High Debt (vs History)", weight, false));
                    } else if de_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(de_z.abs());
                        signals.push(("Low Debt (vs History)", weight, true));
                    }
                } else {
                    // Fall back to absolute thresholds
                    if d2e > 3.0 {
                        signals.push(("High Debt", 3, false));
                    } else if d2e < 0.5 {
                        signals.push(("Low Debt", 2, true));
                    }
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

        // --- Piotroski F-Score (7 of 9 criteria available) ---
        if financials.len() >= 2 {
            let prior = &financials[1];
            let mut f_score: u32 = 0;

            // 1. Positive net income (TTM)
            if ttm_net_income.map_or(false, |ni| ni > 0.0) { f_score += 1; }
            // 2. Positive operating cash flow (TTM)
            if ttm_ocf.map_or(false, |ocf| ocf > 0.0) { f_score += 1; }
            // 3. Cash flow > net income (earnings quality)
            if let (Some(ocf), Some(ni)) = (ttm_ocf, ttm_net_income) {
                if ni > 0.0 && ocf > ni { f_score += 1; }
            }
            // 4. ROA increasing (latest vs prior quarter)
            if let (Some(ni_curr), Some(ta_curr), Some(ni_prior), Some(ta_prior)) =
                (latest.net_income, bs_total_assets, prior.net_income, prior.total_assets) {
                if ta_curr > 0.0 && ta_prior > 0.0 && (ni_curr / ta_curr) > (ni_prior / ta_prior) {
                    f_score += 1;
                }
            }
            // 5. Lower leverage (D/E decreasing)
            if let (Some(tl_curr), Some(eq_curr), Some(tl_prior), Some(eq_prior)) =
                (bs_total_liabilities, bs_shareholders_equity, prior.total_liabilities, prior.shareholders_equity) {
                if eq_curr > 0.0 && eq_prior > 0.0 && (tl_curr / eq_curr) < (tl_prior / eq_prior) {
                    f_score += 1;
                }
            }
            // 6. Higher gross margin
            if let (Some(gp_curr), Some(rev_curr), Some(gp_prior), Some(rev_prior)) =
                (latest.gross_profit, latest.revenue, prior.gross_profit, prior.revenue) {
                if rev_curr > 0.0 && rev_prior > 0.0 && (gp_curr / rev_curr) > (gp_prior / rev_prior) {
                    f_score += 1;
                }
            }
            // 7. Higher asset turnover
            if let (Some(rev_curr), Some(ta_curr), Some(rev_prior), Some(ta_prior)) =
                (latest.revenue, bs_total_assets, prior.revenue, prior.total_assets) {
                if ta_curr > 0.0 && ta_prior > 0.0 && (rev_curr / ta_curr) > (rev_prior / ta_prior) {
                    f_score += 1;
                }
            }

            metrics_map.insert("piotroski_f_score".to_string(), json!(f_score));
            data_fields_present += 1;
            if f_score >= 6 {
                signals.push(("Strong Piotroski F-Score", 3, true));
            } else if f_score >= 4 {
                signals.push(("Moderate Piotroski F-Score", 1, true));
            } else if f_score <= 2 {
                signals.push(("Weak Piotroski F-Score", 2, false));
            }
        }

        // --- Altman Z-Score (approximation — uses equity/assets as WC/TA proxy) ---
        if let (Some(ta), Some(tl), Some(eq), Some(oi), Some(rev)) =
            (bs_total_assets, bs_total_liabilities, bs_shareholders_equity, ttm_operating_income, ttm_revenue) {
            if ta > 0.0 && tl > 0.0 {
                let wc_ta = eq / ta;
                let ebit_ta = oi / ta;
                let sales_ta = rev / ta;
                let mve_tl = if let (Some(p), Some(s)) = (current_price, shares_outstanding) {
                    if p > 0.0 && s > 0.0 { (p * s) / tl } else { 0.0 }
                } else { 0.0 };

                let z_score = 1.2 * wc_ta + 3.3 * ebit_ta + 0.6 * mve_tl + 1.0 * sales_ta;
                metrics_map.insert("altman_z_score".to_string(), json!(z_score));
                data_fields_present += 1;

                if z_score > 2.99 {
                    signals.push(("Safe Zone (Altman Z)", 2, true));
                } else if z_score > 1.81 {
                    signals.push(("Grey Zone (Altman Z)", 1, false));
                } else {
                    signals.push(("Distress Zone (Altman Z)", 3, false));
                }
            }
        }

        // --- DuPont Decomposition: ROE = Net Margin × Asset Turnover × Equity Multiplier ---
        if let (Some(ni), Some(rev), Some(ta), Some(eq)) =
            (ttm_net_income, ttm_revenue, bs_total_assets, bs_shareholders_equity) {
            if rev > 0.0 && ta > 0.0 && eq > 0.0 {
                let net_margin_pct = (ni / rev) * 100.0;
                let asset_turnover = rev / ta;
                let equity_multiplier = ta / eq;
                let dupont_roe = (ni / rev) * asset_turnover * equity_multiplier * 100.0;

                metrics_map.insert("dupont_net_margin".to_string(), json!(net_margin_pct));
                metrics_map.insert("dupont_asset_turnover".to_string(), json!(asset_turnover));
                metrics_map.insert("dupont_equity_multiplier".to_string(), json!(equity_multiplier));
                metrics_map.insert("dupont_roe".to_string(), json!(dupont_roe));
                data_fields_present += 1;

                // Compute historical asset turnover for z-score
                let at_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(r), Some(a)) = (f.revenue, f.total_assets) {
                            if a > 0.0 { Some(r / a) } else { None }
                        } else {
                            None
                        }
                    })
                    .collect();

                if at_history.len() >= 2 {
                    let at_z = adaptive::z_score_of(asset_turnover, &at_history);
                    metrics_map.insert("asset_turnover_z_score".to_string(), json!(at_z));
                    if at_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(at_z.abs());
                        signals.push(("Low Asset Efficiency (vs History)", weight, false));
                    }
                } else if asset_turnover < 0.3 {
                    signals.push(("Low Asset Efficiency", 1, false));
                }

                // Compute historical equity multiplier for z-score
                let em_history: Vec<f64> = financials.iter()
                    .filter_map(|f| {
                        if let (Some(a), Some(e)) = (f.total_assets, f.shareholders_equity) {
                            if e > 0.0 { Some(a / e) } else { None }
                        } else {
                            None
                        }
                    })
                    .collect();

                if em_history.len() >= 2 {
                    let em_z = adaptive::z_score_of(equity_multiplier, &em_history);
                    metrics_map.insert("equity_multiplier_z_score".to_string(), json!(em_z));
                    if em_z > 1.5 {
                        let weight = adaptive::z_score_to_weight(em_z);
                        signals.push(("High Leverage (vs History)", weight, false));
                    }
                } else if equity_multiplier > 5.0 {
                    signals.push(("High Leverage", 2, false));
                }
            }
        }

        // --- Multi-Quarter Trend Analysis ---
        if financials.len() >= 4 {
            // Revenue acceleration: compare recent 2Q growth vs prior 2Q growth
            let q_revenues: Vec<Option<f64>> = financials.iter().take(8).map(|f| f.revenue).collect();
            if q_revenues.len() >= 4 {
                let recent_revs: Vec<f64> = q_revenues[..2].iter().filter_map(|&v| v).collect();
                let prior_revs: Vec<f64> = q_revenues[2..4].iter().filter_map(|&v| v).collect();
                if recent_revs.len() == 2 && prior_revs.len() == 2 && recent_revs[1].abs() > 1.0 && prior_revs[1].abs() > 1.0 {
                    let recent_growth = (recent_revs[0] - recent_revs[1]) / recent_revs[1].abs();
                    let prior_growth = (prior_revs[0] - prior_revs[1]) / prior_revs[1].abs();
                    let acceleration = recent_growth - prior_growth;
                    metrics_map.insert("revenue_acceleration".to_string(), json!(acceleration * 100.0));

                    // Compute historical acceleration values for z-score
                    let mut accel_history: Vec<f64> = Vec::new();
                    for i in 0..financials.len().saturating_sub(4) {
                        let revs: Vec<Option<f64>> = financials[i..i+4].iter().map(|f| f.revenue).collect();
                        if revs.len() == 4 {
                            let r1: Vec<f64> = revs[..2].iter().filter_map(|&v| v).collect();
                            let r2: Vec<f64> = revs[2..4].iter().filter_map(|&v| v).collect();
                            if r1.len() == 2 && r2.len() == 2 && r1[1].abs() > 1.0 && r2[1].abs() > 1.0 {
                                let g1 = (r1[0] - r1[1]) / r1[1].abs();
                                let g2 = (r2[0] - r2[1]) / r2[1].abs();
                                accel_history.push(g1 - g2);
                            }
                        }
                    }

                    if accel_history.len() >= 2 {
                        let accel_z = adaptive::z_score_of(acceleration, &accel_history);
                        metrics_map.insert("revenue_acceleration_z_score".to_string(), json!(accel_z));

                        if accel_z > 1.0 {
                            let weight = adaptive::z_score_to_weight(accel_z);
                            signals.push(("Revenue Accelerating (vs History)", weight, true));
                        } else if accel_z < -1.0 {
                            let weight = adaptive::z_score_to_weight(accel_z.abs());
                            signals.push(("Revenue Decelerating (vs History)", weight, false));
                        }
                    } else {
                        // Fall back to absolute
                        if acceleration > 0.02 {
                            signals.push(("Revenue Accelerating", 2, true));
                        } else if acceleration < -0.05 {
                            signals.push(("Revenue Decelerating", 2, false));
                        }
                    }
                }
            }

            // Margin expansion: compare latest quarter margin vs 4Q-ago margin
            if let (Some(ni_latest), Some(rev_latest)) = (latest.net_income, latest.revenue) {
                if let (Some(ni_prior), Some(rev_prior)) = (financials[3].net_income, financials[3].revenue) {
                    if rev_latest > 0.0 && rev_prior > 0.0 {
                        let margin_latest = ni_latest / rev_latest;
                        let margin_prior = ni_prior / rev_prior;
                        let margin_change = (margin_latest - margin_prior) * 100.0;
                        metrics_map.insert("margin_expansion_yoy".to_string(), json!(margin_change));

                        // Compute historical margin changes for z-score
                        let mut margin_change_history: Vec<f64> = Vec::new();
                        for i in 0..financials.len().saturating_sub(4) {
                            if let (Some(ni_curr), Some(rev_curr)) = (financials[i].net_income, financials[i].revenue) {
                                if let (Some(ni_prior_q), Some(rev_prior_q)) = (financials[i+3].net_income, financials[i+3].revenue) {
                                    if rev_curr > 0.0 && rev_prior_q > 0.0 {
                                        let m_curr = ni_curr / rev_curr;
                                        let m_prior = ni_prior_q / rev_prior_q;
                                        margin_change_history.push((m_curr - m_prior) * 100.0);
                                    }
                                }
                            }
                        }

                        if margin_change_history.len() >= 2 {
                            let margin_change_z = adaptive::z_score_of(margin_change, &margin_change_history);
                            metrics_map.insert("margin_expansion_z_score".to_string(), json!(margin_change_z));

                            if margin_change_z > 1.0 {
                                let weight = adaptive::z_score_to_weight(margin_change_z);
                                signals.push(("Margin Expanding (vs History)", weight, true));
                            } else if margin_change_z < -1.0 {
                                let weight = adaptive::z_score_to_weight(margin_change_z.abs());
                                signals.push(("Margin Contracting (vs History)", weight, false));
                            }
                        } else {
                            // Fall back to absolute
                            if margin_change > 2.0 {
                                signals.push(("Margin Expanding", 2, true));
                            } else if margin_change < -3.0 {
                                signals.push(("Margin Contracting", 2, false));
                            }
                        }
                    }
                }
            }

            // Consecutive revenue beats (quarters of positive sequential growth)
            let mut consecutive_growth = 0u32;
            for i in 0..financials.len().min(4).saturating_sub(1) {
                if let (Some(r_curr), Some(r_prior)) = (financials[i].revenue, financials[i + 1].revenue) {
                    if r_prior > 0.0 && r_curr > r_prior {
                        consecutive_growth += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            if consecutive_growth >= 3 {
                signals.push(("Consistent Revenue Growth", 2, true));
            }
            metrics_map.insert("consecutive_growth_quarters".to_string(), json!(consecutive_growth));
        }

        // --- EV/EBITDA (growth-implied fair value) ---
        // EBITDA ≈ operating income (D&A not separately available from Polygon)
        // EV ≈ Market Cap + Total Liabilities (cash not separately available)
        if let (Some(oi), Some(tl)) = (ttm_operating_income, bs_total_liabilities) {
            if let (Some(p), Some(s)) = (current_price, shares_outstanding) {
                if p > 0.0 && s > 0.0 && oi > 0.0 {
                    let market_cap = p * s;
                    let ev = market_cap + tl;
                    let ev_ebitda = ev / oi;
                    metrics_map.insert("ev_ebitda".to_string(), json!(ev_ebitda));
                    data_fields_present += 1;

                    // Use growth-implied fair EV/EBITDA (approximation from implied P/E)
                    let growth_rate = revenue_growth.unwrap_or(0.0) / 100.0;
                    let rf = risk_free_rate.unwrap_or(0.045);
                    let equity_risk_premium = 0.055;
                    let implied_pe = (1.0 / (rf + equity_risk_premium - growth_rate.min(0.08)))
                        .max(5.0)
                        .min(80.0);
                    let implied_ev_ebitda = implied_pe * 0.7; // Rough approximation
                    let ev_ebitda_z = (ev_ebitda - implied_ev_ebitda) / implied_ev_ebitda.max(1.0);

                    metrics_map.insert("implied_ev_ebitda".to_string(), json!(implied_ev_ebitda));
                    metrics_map.insert("ev_ebitda_z_score".to_string(), json!(ev_ebitda_z));

                    if ev_ebitda_z < -0.3 {
                        let weight = adaptive::z_score_to_weight(ev_ebitda_z.abs());
                        signals.push(("Low EV/EBITDA (vs Growth-Implied)", weight, true));
                    } else if ev_ebitda_z > 0.5 {
                        let weight = adaptive::z_score_to_weight(ev_ebitda_z);
                        signals.push(("High EV/EBITDA (vs Growth-Implied)", weight, false));
                    }
                }
            }
        }

        // --- Accrual Ratio (Earnings Quality Measure) ---
        // Accrual Ratio = (Net Income - OCF) / Total Assets
        // Higher accruals = lower earnings quality (potential manipulation)
        if let (Some(ni), Some(ocf), Some(ta)) = (ttm_net_income, ttm_ocf, bs_total_assets) {
            if ta > 0.0 {
                let accrual_ratio = (ni - ocf) / ta;
                metrics_map.insert("accrual_ratio".to_string(), json!(accrual_ratio));
                data_fields_present += 1;

                // Compute historical accrual ratios for z-score
                let mut accrual_history: Vec<f64> = Vec::new();
                for i in 0..financials.len().saturating_sub(4) {
                    let hist_ni: f64 = financials[i..i.min(i+4)].iter().filter_map(|f| f.net_income).sum();
                    let hist_ocf: f64 = financials[i..i.min(i+4)].iter().filter_map(|f| f.cash_flow_operating).sum();
                    if let Some(hist_ta) = financials[i].total_assets {
                        if hist_ta > 0.0 {
                            accrual_history.push((hist_ni - hist_ocf) / hist_ta);
                        }
                    }
                }

                if accrual_history.len() >= 2 {
                    let accrual_z = adaptive::z_score_of(accrual_ratio, &accrual_history);
                    metrics_map.insert("accrual_ratio_z_score".to_string(), json!(accrual_z));

                    if accrual_z.abs() > 1.5 {
                        let weight = adaptive::z_score_to_weight(accrual_z.abs());
                        if accrual_z > 1.5 {
                            signals.push(("High Accruals (Earnings Quality Risk)", weight, false));
                        } else {
                            signals.push(("Low Accruals (High Quality)", weight, true));
                        }
                    }
                } else {
                    // Fall back to absolute thresholds
                    if accrual_ratio.abs() > 0.08 {
                        signals.push(("High Accruals (Earnings Quality Risk)", 2, false));
                    } else if accrual_ratio.abs() < 0.02 {
                        signals.push(("Low Accruals (High Quality)", 1, true));
                    }
                }
            }
        }

        // --- Working Capital Efficiency (Working Capital Turnover) ---
        // Measures how efficiently company uses working capital to generate revenue
        if let (Some(rev), Some(ta), Some(tl)) = (ttm_revenue, bs_total_assets, bs_total_liabilities) {
            // Approximate working capital as net current assets (Total Assets - Total Liabilities)
            // Note: Polygon doesn't provide current assets/liabilities separately
            let working_capital = ta - tl;
            if working_capital > 0.0 {
                let wc_turnover = rev / working_capital;
                metrics_map.insert("working_capital_turnover".to_string(), json!(wc_turnover));
                data_fields_present += 1;

                // Compute historical WC turnover for z-score
                let mut wc_turnover_history: Vec<f64> = Vec::new();
                for f in financials.iter() {
                    if let (Some(f_rev), Some(f_ta), Some(f_tl)) = (f.revenue, f.total_assets, f.total_liabilities) {
                        let f_wc = f_ta - f_tl;
                        if f_wc > 0.0 {
                            wc_turnover_history.push(f_rev / f_wc);
                        }
                    }
                }

                if wc_turnover_history.len() >= 2 {
                    let wc_z = adaptive::z_score_of(wc_turnover, &wc_turnover_history);
                    metrics_map.insert("working_capital_turnover_z_score".to_string(), json!(wc_z));

                    if wc_z > 1.0 {
                        let weight = adaptive::z_score_to_weight(wc_z);
                        signals.push(("Efficient Working Capital Use", weight, true));
                    } else if wc_z < -1.0 {
                        let weight = adaptive::z_score_to_weight(wc_z.abs());
                        signals.push(("Inefficient Working Capital Use", weight, false));
                    }
                } else {
                    // Fall back to absolute thresholds (sector-dependent, use conservative)
                    if wc_turnover > 4.0 {
                        signals.push(("Efficient Working Capital Use", 1, true));
                    } else if wc_turnover < 1.5 {
                        signals.push(("Inefficient Working Capital Use", 1, false));
                    }
                }
            }
        }

        // --- Beneish M-Score (Earnings Manipulation Detection) ---
        // Simplified 5-variable model (requires at least 2 quarters for comparison)
        if financials.len() >= 2 {
            let prior = &financials[1];
            // Variables: DSRI, GMI, AQI, SGI, DEPI (we can compute some with available data)

            // DSRI: Days Sales in Receivables Index (requires receivables — not available)
            // GMI: Gross Margin Index
            let gmi = if let (Some(gp_curr), Some(rev_curr), Some(gp_prior), Some(rev_prior)) =
                (latest.gross_profit, latest.revenue, prior.gross_profit, prior.revenue) {
                if rev_curr > 0.0 && rev_prior > 0.0 {
                    let gm_prior = gp_prior / rev_prior;
                    let gm_curr = gp_curr / rev_curr;
                    if gm_curr > 0.0 { Some(gm_prior / gm_curr) } else { None }
                } else { None }
            } else { None };

            // AQI: Asset Quality Index (requires current assets breakdown — not available)
            // SGI: Sales Growth Index
            let sgi = if let (Some(rev_curr), Some(rev_prior)) = (latest.revenue, prior.revenue) {
                if rev_prior > 0.0 { Some(rev_curr / rev_prior) } else { None }
            } else { None };

            // DEPI: Depreciation Index (requires D&A — not available from Polygon)

            // With limited variables, compute a partial M-Score using GMI and SGI
            if let (Some(gmi_val), Some(sgi_val)) = (gmi, sgi) {
                // Partial M-Score approximation (coefficients from Beneish 1999, scaled)
                let m_score_partial = -4.84 + 0.92 * (gmi_val - 1.0) + 0.528 * (sgi_val - 1.0);
                metrics_map.insert("beneish_m_score_partial".to_string(), json!(m_score_partial));
                data_fields_present += 1;

                // M-Score > -2.22 suggests potential manipulation
                if m_score_partial > -2.22 {
                    signals.push(("Elevated Manipulation Risk (Beneish)", 3, false));
                } else if m_score_partial > -2.5 {
                    signals.push(("Moderate Manipulation Risk (Beneish)", 1, false));
                }
            }
        }

        // --- Dividend Safety Score (if we have dividend data from supplementary signals) ---
        // This will be computed later if dividend data is available
        // Placeholder: we'll add this to the metrics map if data is present

        // --- Financing Cash Flow Analysis (buybacks, debt issuance, capital structure) ---
        if let Some(cff) = ttm_cff {
            metrics_map.insert("financing_cash_flow".to_string(), json!(cff));
            data_fields_present += 1;

            // Large negative financing CF often means buybacks or debt repayment (shareholder-friendly)
            if cff < 0.0 {
                if let Some(ocf) = ttm_ocf {
                    if ocf > 0.0 {
                        let buyback_intensity = (-cff) / ocf;
                        metrics_map.insert("buyback_intensity".to_string(), json!(buyback_intensity));

                        // Compute historical buyback intensity for z-score
                        let mut intensity_history: Vec<f64> = Vec::new();
                        for i in 0..financials.len().saturating_sub(4) {
                            let hist_cff: f64 = financials[i..i.min(i+4)].iter()
                                .filter_map(|f| f.cash_flow_financing).sum();
                            let hist_ocf: f64 = financials[i..i.min(i+4)].iter()
                                .filter_map(|f| f.cash_flow_operating).sum();
                            if hist_cff < 0.0 && hist_ocf > 0.0 {
                                intensity_history.push((-hist_cff) / hist_ocf);
                            }
                        }

                        if intensity_history.len() >= 2 {
                            let intensity_z = adaptive::z_score_of(buyback_intensity, &intensity_history);
                            metrics_map.insert("buyback_intensity_z_score".to_string(), json!(intensity_z));
                            if intensity_z > 1.0 {
                                let weight = adaptive::z_score_to_weight(intensity_z);
                                signals.push(("Aggressive Buyback/Debt Repay (vs History)", weight, true));
                            }
                        } else {
                            // Fall back to absolute
                            if buyback_intensity > 0.5 {
                                signals.push(("Aggressive Buyback/Debt Repay", 2, true));
                            } else if buyback_intensity > 0.25 {
                                signals.push(("Active Capital Return", 1, true));
                            }
                        }
                    }
                }
            } else if cff > 0.0 {
                // Positive financing CF = raising capital (debt or equity issuance)
                if let Some(tl) = bs_total_liabilities {
                    if tl > 0.0 {
                        let raise_ratio = cff / tl;
                        if raise_ratio > 0.1 {
                            signals.push(("Significant Capital Raise", 2, false));
                        }
                    }
                }
                // Raising while operating CF is negative = red flag
                if ttm_ocf.map_or(false, |ocf| ocf < 0.0) {
                    signals.push(("Capital Raise + Negative OCF", 3, false));
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
                    let ptfv = price / fair_value;
                    metrics_map.insert("price_to_fair_value".to_string(), json!(ptfv));

                    // Use z-score of price-to-fair-value ratio (deviations from 1.0)
                    // Typical std dev is ~0.3 (30% deviation from fair value)
                    let ptfv_z = (ptfv - 1.0) / 0.3;
                    metrics_map.insert("price_to_fair_value_z_score".to_string(), json!(ptfv_z));

                    if ptfv_z > 1.5 {
                        let weight = adaptive::z_score_to_weight(ptfv_z);
                        signals.push(("Significantly Above Fair Value", weight, false));
                    } else if ptfv_z > 0.6 {
                        let weight = adaptive::z_score_to_weight(ptfv_z);
                        signals.push(("Trading Above Fair Value", weight, false));
                    } else if ptfv_z < -1.5 {
                        let weight = adaptive::z_score_to_weight(ptfv_z.abs());
                        signals.push(("Significantly Below Fair Value", weight, true));
                    } else if ptfv_z < -0.6 {
                        let weight = adaptive::z_score_to_weight(ptfv_z.abs());
                        signals.push(("Trading Below Fair Value", weight, true));
                    }
                }
            }
        }

        // --- Fundamental Value Score: quality + valuation ---
        // Composite quality score from existing metrics using z-scores where available
        let has_strong_roe = if let Some(roe_z) = metrics_map.get("roe_z_score").and_then(|v| v.as_f64()) {
            roe_z > 0.5
        } else {
            metrics_map.get("roe").and_then(|v| v.as_f64()).map_or(false, |r| r > 12.0)
        };

        let has_high_margins = if let Some(pm_z) = metrics_map.get("profit_margin_z_score").and_then(|v| v.as_f64()) {
            pm_z > 0.5
        } else {
            metrics_map.get("profit_margin").and_then(|v| v.as_f64()).map_or(false, |m| m > 12.0)
        };

        let has_positive_cf = ttm_ocf.map_or(false, |ocf| ocf > 0.0);

        let has_strong_roic = metrics_map.get("roic").and_then(|v| v.as_f64()).map_or(false, |r| r > 10.0);

        let has_rev_growth = if let Some(rg_z) = metrics_map.get("revenue_growth_z_score").and_then(|v| v.as_f64()) {
            rg_z > 0.5
        } else {
            revenue_growth.map_or(false, |g| g > 5.0)
        };

        let has_low_debt = if let Some(de_z) = metrics_map.get("debt_to_equity_z_score").and_then(|v| v.as_f64()) {
            de_z < 0.0
        } else {
            metrics_map.get("debt_to_equity").and_then(|v| v.as_f64()).map_or(false, |d| d < 1.5)
        };

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

        // Revenue Growth — adaptive based on historical z-score
        if let Some(growth) = revenue_growth {
            metrics_map.insert("revenue_growth".to_string(), json!(growth));
            data_fields_present += 1;

            // Compute historical growth rates across available quarters
            let mut growth_history: Vec<f64> = Vec::new();
            for i in 0..financials.len().saturating_sub(5) {
                let current_ttm: f64 = financials[i..i.min(i+4)].iter().filter_map(|f| f.revenue).sum();
                let prior_ttm: f64 = financials[i+4..financials.len().min(i+8)].iter().filter_map(|f| f.revenue).sum();
                if current_ttm > 0.0 && prior_ttm > 0.0 {
                    growth_history.push(((current_ttm - prior_ttm) / prior_ttm) * 100.0);
                }
            }

            if growth_history.len() >= 2 {
                let growth_z = adaptive::z_score_of(growth, &growth_history);
                metrics_map.insert("revenue_growth_z_score".to_string(), json!(growth_z));

                if growth_z > 1.0 {
                    let weight = adaptive::z_score_to_weight(growth_z);
                    signals.push(("Strong Revenue Growth (vs History)", weight, true));
                } else if growth_z < -1.0 {
                    let weight = adaptive::z_score_to_weight(growth_z.abs());
                    signals.push(("Revenue Decline (vs History)", weight, false));
                }
            } else {
                // Fall back to moderate absolute thresholds
                if growth > 8.0 {
                    signals.push(("Strong Revenue Growth", 3, true));
                } else if growth < -3.0 {
                    signals.push(("Revenue Decline", 3, false));
                }
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
        sic_description: Option<&str>,
    ) -> Result<AnalysisResult, AnalysisError> {
        let mut result = self.analyze_enhanced(symbol, financials, current_price, shares_outstanding, risk_free_rate, sic_description)?;

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
