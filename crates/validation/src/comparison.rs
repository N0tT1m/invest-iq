use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::alpha_vantage::AlphaVantageClient;
use crate::yahoo_finance::YahooFinanceClient;

#[derive(Clone)]
pub struct ComparisonEngine {
    alpha_vantage: AlphaVantageClient,
    yahoo_finance: YahooFinanceClient,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub symbol: String,
    pub technical_comparison: TechnicalComparison,
    pub fundamental_comparison: FundamentalComparison,
    pub overall_accuracy: f64,
    pub differences_summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TechnicalComparison {
    pub rsi_difference: Option<IndicatorDifference>,
    pub macd_difference: Option<MACDDifference>,
    pub sma_difference: Option<IndicatorDifference>,
    pub overall_technical_accuracy: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndicatorDifference {
    pub our_value: f64,
    pub their_value: f64,
    pub absolute_difference: f64,
    pub percentage_difference: f64,
    pub within_tolerance: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MACDDifference {
    pub macd: IndicatorDifference,
    pub signal: IndicatorDifference,
    pub histogram: IndicatorDifference,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundamentalComparison {
    pub pe_ratio_difference: Option<IndicatorDifference>,
    pub roe_difference: Option<IndicatorDifference>,
    pub profit_margin_difference: Option<IndicatorDifference>,
    pub debt_to_equity_difference: Option<IndicatorDifference>,
    pub beta_difference: Option<IndicatorDifference>,
    pub overall_fundamental_accuracy: f64,
}

impl ComparisonEngine {
    pub fn new(alpha_vantage_key: String) -> Self {
        Self {
            alpha_vantage: AlphaVantageClient::new(alpha_vantage_key),
            yahoo_finance: YahooFinanceClient::new(),
        }
    }

    /// Compare our RSI with Alpha Vantage's RSI
    pub async fn compare_rsi(
        &self,
        symbol: &str,
        our_rsi: f64,
        interval: &str,
        period: u32,
    ) -> Result<IndicatorDifference> {
        let av_data = self.alpha_vantage.get_rsi(symbol, interval, period).await?;

        let their_rsi = av_data.first()
            .map(|d| d.value)
            .unwrap_or(0.0);

        Ok(Self::calculate_difference(our_rsi, their_rsi, 2.0))
    }

    /// Compare our MACD with Alpha Vantage's MACD
    pub async fn compare_macd(
        &self,
        symbol: &str,
        our_macd: f64,
        our_signal: f64,
        our_histogram: f64,
        interval: &str,
    ) -> Result<MACDDifference> {
        let av_data = self.alpha_vantage.get_macd(symbol, interval).await?;

        let first_data = av_data.first();

        let their_macd = first_data.map(|d| d.macd).unwrap_or(0.0);
        let their_signal = first_data.map(|d| d.signal).unwrap_or(0.0);
        let their_histogram = first_data.map(|d| d.histogram).unwrap_or(0.0);

        Ok(MACDDifference {
            macd: Self::calculate_difference(our_macd, their_macd, 0.5),
            signal: Self::calculate_difference(our_signal, their_signal, 0.5),
            histogram: Self::calculate_difference(our_histogram, their_histogram, 0.5),
        })
    }

    /// Compare our SMA with Alpha Vantage's SMA
    pub async fn compare_sma(
        &self,
        symbol: &str,
        our_sma: f64,
        interval: &str,
        period: u32,
    ) -> Result<IndicatorDifference> {
        let av_data = self.alpha_vantage.get_sma(symbol, interval, period).await?;

        let their_sma = av_data.first()
            .map(|d| d.value)
            .unwrap_or(0.0);

        Ok(Self::calculate_difference(our_sma, their_sma, 1.0))
    }

    /// Compare fundamental data with Yahoo Finance
    pub async fn compare_fundamentals(
        &self,
        symbol: &str,
        our_pe: Option<f64>,
        our_roe: Option<f64>,
        our_profit_margin: Option<f64>,
        our_debt_to_equity: Option<f64>,
        our_beta: Option<f64>,
    ) -> Result<FundamentalComparison> {
        let yahoo_data = self.yahoo_finance.get_fundamentals(symbol).await?;

        let pe_diff = if let (Some(our), Some(their)) = (our_pe, yahoo_data.pe_ratio) {
            Some(Self::calculate_difference(our, their, 5.0))
        } else {
            None
        };

        let roe_diff = if let (Some(our), Some(their)) = (our_roe, yahoo_data.return_on_equity) {
            Some(Self::calculate_difference(our, their * 100.0, 5.0)) // Yahoo returns as decimal
        } else {
            None
        };

        let margin_diff = if let (Some(our), Some(their)) = (our_profit_margin, yahoo_data.profit_margin) {
            Some(Self::calculate_difference(our, their * 100.0, 5.0)) // Yahoo returns as decimal
        } else {
            None
        };

        let debt_diff = if let (Some(our), Some(their)) = (our_debt_to_equity, yahoo_data.debt_to_equity) {
            Some(Self::calculate_difference(our, their, 10.0))
        } else {
            None
        };

        let beta_diff = if let (Some(our), Some(their)) = (our_beta, yahoo_data.beta) {
            Some(Self::calculate_difference(our, their, 0.2))
        } else {
            None
        };

        // Calculate overall accuracy
        let mut total_accuracy = 0.0;
        let mut count = 0;

        if let Some(diff) = &pe_diff {
            total_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs() };
            count += 1;
        }
        if let Some(diff) = &roe_diff {
            total_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            count += 1;
        }
        if let Some(diff) = &margin_diff {
            total_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            count += 1;
        }
        if let Some(diff) = &debt_diff {
            total_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            count += 1;
        }
        if let Some(diff) = &beta_diff {
            total_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            count += 1;
        }

        let overall_fundamental_accuracy = if count > 0 {
            total_accuracy / count as f64
        } else {
            0.0
        };

        Ok(FundamentalComparison {
            pe_ratio_difference: pe_diff,
            roe_difference: roe_diff,
            profit_margin_difference: margin_diff,
            debt_to_equity_difference: debt_diff,
            beta_difference: beta_diff,
            overall_fundamental_accuracy,
        })
    }

    /// Helper function to calculate the difference between two values
    fn calculate_difference(our_value: f64, their_value: f64, tolerance_percent: f64) -> IndicatorDifference {
        let absolute_difference = (our_value - their_value).abs();

        let percentage_difference = if their_value != 0.0 {
            ((our_value - their_value) / their_value) * 100.0
        } else {
            0.0
        };

        let within_tolerance = percentage_difference.abs() <= tolerance_percent;

        IndicatorDifference {
            our_value,
            their_value,
            absolute_difference,
            percentage_difference,
            within_tolerance,
        }
    }

    /// Perform a full comparison
    pub async fn full_comparison(
        &self,
        symbol: &str,
        our_rsi: Option<f64>,
        our_macd: Option<(f64, f64, f64)>, // (macd, signal, histogram)
        our_sma_20: Option<f64>,
        our_pe: Option<f64>,
        our_roe: Option<f64>,
        our_profit_margin: Option<f64>,
        our_debt_to_equity: Option<f64>,
        our_beta: Option<f64>,
    ) -> Result<ComparisonResult> {
        // Technical comparisons
        let rsi_diff = if let Some(rsi) = our_rsi {
            self.compare_rsi(symbol, rsi, "daily", 14).await.ok()
        } else {
            None
        };

        let macd_diff = if let Some((macd, signal, hist)) = our_macd {
            self.compare_macd(symbol, macd, signal, hist, "daily").await.ok()
        } else {
            None
        };

        let sma_diff = if let Some(sma) = our_sma_20 {
            self.compare_sma(symbol, sma, "daily", 20).await.ok()
        } else {
            None
        };

        // Calculate technical accuracy
        let mut tech_accuracy = 0.0;
        let mut tech_count = 0;

        if let Some(diff) = &rsi_diff {
            tech_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            tech_count += 1;
        }
        if let Some(diff) = &macd_diff {
            let macd_acc = if diff.macd.within_tolerance { 100.0 } else { 100.0 - diff.macd.percentage_difference.abs().min(100.0) };
            tech_accuracy += macd_acc;
            tech_count += 1;
        }
        if let Some(diff) = &sma_diff {
            tech_accuracy += if diff.within_tolerance { 100.0 } else { 100.0 - diff.percentage_difference.abs().min(100.0) };
            tech_count += 1;
        }

        let overall_technical_accuracy = if tech_count > 0 {
            tech_accuracy / tech_count as f64
        } else {
            0.0
        };

        let technical_comparison = TechnicalComparison {
            rsi_difference: rsi_diff,
            macd_difference: macd_diff,
            sma_difference: sma_diff,
            overall_technical_accuracy,
        };

        // Fundamental comparison
        let fundamental_comparison = self.compare_fundamentals(
            symbol,
            our_pe,
            our_roe,
            our_profit_margin,
            our_debt_to_equity,
            our_beta,
        ).await?;

        // Overall accuracy
        let overall_accuracy = (overall_technical_accuracy + fundamental_comparison.overall_fundamental_accuracy) / 2.0;

        // Generate summary
        let differences_summary = Self::generate_summary(&technical_comparison, &fundamental_comparison);

        Ok(ComparisonResult {
            symbol: symbol.to_string(),
            technical_comparison,
            fundamental_comparison,
            overall_accuracy,
            differences_summary,
        })
    }

    fn generate_summary(tech: &TechnicalComparison, fund: &FundamentalComparison) -> String {
        let mut summary = Vec::new();

        if let Some(diff) = &tech.rsi_difference {
            if !diff.within_tolerance {
                summary.push(format!("RSI differs by {:.1}%", diff.percentage_difference.abs()));
            }
        }

        if let Some(diff) = &tech.macd_difference {
            if !diff.macd.within_tolerance {
                summary.push(format!("MACD differs by {:.1}%", diff.macd.percentage_difference.abs()));
            }
        }

        if let Some(diff) = &fund.pe_ratio_difference {
            if !diff.within_tolerance {
                summary.push(format!("P/E ratio differs by {:.1}%", diff.percentage_difference.abs()));
            }
        }

        if summary.is_empty() {
            "All metrics within acceptable tolerance".to_string()
        } else {
            summary.join("; ")
        }
    }
}
