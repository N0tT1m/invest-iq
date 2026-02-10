use analysis_core::{AnalysisError, AnalysisResult, Bar, SignalStrength, TechnicalAnalyzer};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use crate::indicators::*;
use crate::patterns::*;

pub struct TechnicalAnalysisEngine;

fn pattern_name(p: &crate::patterns::CandlestickPattern) -> &'static str {
    match p {
        CandlestickPattern::Doji => "Doji",
        CandlestickPattern::Hammer => "Hammer",
        CandlestickPattern::InvertedHammer => "Inverted Hammer",
        CandlestickPattern::ShootingStar => "Shooting Star",
        CandlestickPattern::Engulfing => "Engulfing",
        CandlestickPattern::Piercing => "Piercing",
        CandlestickPattern::DarkCloudCover => "Dark Cloud Cover",
        CandlestickPattern::MorningStar => "Morning Star",
        CandlestickPattern::EveningStar => "Evening Star",
        CandlestickPattern::ThreeWhiteSoldiers => "Three White Soldiers",
        CandlestickPattern::ThreeBlackCrows => "Three Black Crows",
    }
}

/// Shared signal data computed from bars
struct SignalData {
    signals: Vec<(&'static str, i32, bool)>,
    rsi_values: Vec<f64>,
    macd_result: MacdResult,
    sma_20: Vec<f64>,
    sma_50: Vec<f64>,
    bb: BollingerBands,
    patterns: Vec<PatternMatch>,
    trend: Trend,
    closes: Vec<f64>,
    adx_value: Option<f64>,
    support: Option<f64>,
    resistance: Option<f64>,
}

impl TechnicalAnalysisEngine {
    pub fn new() -> Self {
        Self
    }

    /// Build the core set of signals shared by both analyze_sync and analyze_enhanced
    fn build_signals(&self, bars: &[Bar]) -> Result<SignalData, AnalysisError> {
        if bars.len() < 50 {
            return Err(AnalysisError::InsufficientData(
                "Need at least 50 bars for technical analysis".to_string(),
            ));
        }

        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let mut signals: Vec<(&'static str, i32, bool)> = Vec::new();

        // RSI Analysis (deeper oversold = stronger reversal signal)
        let rsi_values = rsi(&closes, 14);
        if let Some(&last_rsi) = rsi_values.last() {
            if last_rsi < 25.0 {
                signals.push(("RSI Deeply Oversold", 3, true));
            } else if last_rsi < 30.0 {
                signals.push(("RSI Oversold", 2, true));
            } else if last_rsi > 70.0 {
                signals.push(("RSI Overbought", 2, false));
            }
        }

        // MACD Analysis
        let macd_result = macd(&closes, 12, 26, 9);
        if !macd_result.histogram.is_empty() {
            let last_hist = macd_result.histogram.last().unwrap();
            if macd_result.histogram.len() > 1 {
                let prev_hist = macd_result.histogram[macd_result.histogram.len() - 2];
                if *last_hist > 0.0 && prev_hist <= 0.0 {
                    signals.push(("MACD Bullish Cross", 3, true));
                }
                if *last_hist < 0.0 && prev_hist >= 0.0 {
                    signals.push(("MACD Bearish Cross", 3, false));
                }
            }
        }

        // Moving Average Analysis
        let sma_20 = sma(&closes, 20);
        let sma_50 = sma(&closes, 50);
        if !sma_20.is_empty() && !sma_50.is_empty() {
            let last_sma_20 = sma_20.last().unwrap();
            let last_sma_50 = sma_50.last().unwrap();
            let current_price = closes.last().unwrap();

            if current_price > last_sma_20 && current_price > last_sma_50 {
                signals.push(("Price Above MAs", 2, true));
            }
            if current_price < last_sma_20 && current_price < last_sma_50 {
                signals.push(("Price Below MAs", 2, false));
            }

            if sma_20.len() > 1 && sma_50.len() > 1 {
                let prev_sma_20 = sma_20[sma_20.len() - 2];
                let prev_sma_50 = sma_50[sma_50.len() - 2];

                if last_sma_20 > last_sma_50 && prev_sma_20 <= prev_sma_50 {
                    signals.push(("Golden Cross", 4, true));
                }
                if last_sma_20 < last_sma_50 && prev_sma_20 >= prev_sma_50 {
                    signals.push(("Death Cross", 4, false));
                }
            }
        }

        // Bollinger Bands
        let bb = bollinger_bands(&closes, 20, 2.0);
        if !bb.upper.is_empty() {
            let current_price = closes.last().unwrap();
            let upper = bb.upper.last().unwrap();
            let lower = bb.lower.last().unwrap();

            if current_price < lower {
                signals.push(("Below Lower BB", 2, true));
            } else if current_price > upper {
                signals.push(("Above Upper BB", 2, false));
            }
        }

        // Stochastic Oscillator
        let stoch = stochastic(bars, 14, 3);
        if !stoch.k.is_empty() {
            let last_k = stoch.k.last().unwrap();
            if *last_k < 20.0 {
                signals.push(("Stochastic Oversold", 2, true));
            } else if *last_k > 80.0 {
                signals.push(("Stochastic Overbought", 2, false));
            }
        }

        // Pattern Detection
        let patterns = detect_patterns(bars);
        for pattern in &patterns {
            let weight = (pattern.strength * 3.0) as i32;
            signals.push((pattern_name(&pattern.pattern), weight, pattern.bullish));
        }

        // ADX Trend Strength
        let adx_result = adx(bars, 14);
        let last_adx = adx_result.adx.last().copied();
        if let Some(adx_val) = last_adx {
            if adx_val > 25.0 {
                // Strong trend — check +DI vs -DI for direction
                let last_pdi = adx_result.plus_di.last().copied().unwrap_or(0.0);
                let last_mdi = adx_result.minus_di.last().copied().unwrap_or(0.0);
                if last_pdi > last_mdi {
                    signals.push(("Strong Bullish Trend (ADX)", 3, true));
                } else {
                    signals.push(("Strong Bearish Trend (ADX)", 3, false));
                }
            }
            // ADX < 20 = weak/no trend — don't add signal, just report in metrics
        }

        // Support / Resistance proximity
        let sr = support_resistance(bars, 30.min(bars.len()));
        let current_price = *closes.last().unwrap();
        if current_price > 0.0 {
            if let Some(support) = sr.support {
                let distance_pct = (current_price - support) / current_price * 100.0;
                if distance_pct < 2.0 {
                    signals.push(("Near Support Level", 2, true));
                }
            }
            if let Some(resistance) = sr.resistance {
                let distance_pct = (resistance - current_price) / current_price * 100.0;
                if distance_pct < 2.0 {
                    signals.push(("Near Resistance Level", 2, false));
                }
            }
        }

        // Trend Detection
        let trend = detect_trend(bars, 20);
        match trend {
            Trend::Uptrend => signals.push(("Uptrend", 2, true)),
            Trend::Downtrend => signals.push(("Downtrend", 2, false)),
            Trend::Sideways => {}
        }

        Ok(SignalData {
            signals,
            rsi_values,
            macd_result,
            sma_20,
            sma_50,
            bb,
            patterns,
            trend,
            closes,
            adx_value: last_adx,
            support: sr.support,
            resistance: sr.resistance,
        })
    }

    /// Build metrics JSON from signal data and optional enhanced metrics
    fn build_metrics(
        &self,
        data: &SignalData,
        extra: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let detected_patterns: Vec<serde_json::Value> = data.patterns.iter().map(|p| {
            json!({
                "name": pattern_name(&p.pattern),
                "index": p.index,
                "strength": p.strength,
                "bullish": p.bullish,
            })
        }).collect();

        let (bb_width_val, bb_percent_b_val) = if !data.bb.upper.is_empty() && !data.bb.middle.is_empty() && !data.bb.lower.is_empty() {
            let upper = *data.bb.upper.last().unwrap();
            let middle = *data.bb.middle.last().unwrap();
            let lower = *data.bb.lower.last().unwrap();
            let current_price = *data.closes.last().unwrap();
            let width = if middle != 0.0 { (upper - lower) / middle } else { 0.0 };
            let percent_b = if (upper - lower) != 0.0 { (current_price - lower) / (upper - lower) } else { 0.5 };
            (Some(width), Some(percent_b))
        } else {
            (None, None)
        };

        let mut metrics = json!({
            "rsi": data.rsi_values.last(),
            "macd_histogram": data.macd_result.histogram.last(),
            "trend": format!("{:?}", data.trend),
            "patterns": data.patterns.len(),
            "detected_patterns": detected_patterns,
            "signal_count": data.signals.len(),
            "bb_width": bb_width_val,
            "bb_percent_b": bb_percent_b_val,
            "sma_20": data.sma_20.last(),
            "sma_50": data.sma_50.last(),
            "adx": data.adx_value,
            "support": data.support,
            "resistance": data.resistance,
        });

        // Merge extra metrics if provided
        if let Some(extra) = extra {
            if let (Some(base_map), Some(extra_map)) = (metrics.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_map {
                    base_map.insert(k.clone(), v.clone());
                }
            }
        }

        metrics
    }

    fn analyze_sync(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError> {
        let data = self.build_signals(bars)?;

        let mut total_score = 0;
        let mut total_weight = 0;
        for (_, weight, bullish) in &data.signals {
            total_weight += weight;
            total_score += if *bullish { *weight } else { -weight };
        }

        let normalized_score = if total_weight > 0 {
            (total_score as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };

        let signal = SignalStrength::from_score(normalized_score as i32);
        let confidence = (total_weight as f64 / 20.0).min(1.0);

        let reason = data.signals
            .iter()
            .map(|(name, _, bullish)| {
                format!("{} {}", if *bullish { "+" } else { "-" }, name)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let metrics = self.build_metrics(&data, None);

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason,
            metrics,
        })
    }

    /// Enhanced technical analysis with 5 additional signal types
    pub fn analyze_enhanced(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError> {
        let mut data = self.build_signals(bars)?;

        let closes = &data.closes;
        let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();

        // --- Enhanced Signal 1: RSI Divergence ---
        let mut rsi_divergence: Option<&str> = None;
        if data.rsi_values.len() >= 20 && closes.len() >= 20 {
            // Compare last 20 bars: find two most recent peaks/troughs
            let n = data.rsi_values.len();
            let price_tail = &closes[closes.len().saturating_sub(n)..];
            // Find local peaks (simple: compare with neighbors)
            let mut price_peaks: Vec<(usize, f64)> = Vec::new();
            let mut rsi_peaks: Vec<(usize, f64)> = Vec::new();
            let window = n.min(20);
            let start = n.saturating_sub(window);
            for i in (start + 1)..(n - 1) {
                if price_tail[i] > price_tail[i - 1] && price_tail[i] > price_tail[i + 1] {
                    price_peaks.push((i, price_tail[i]));
                }
                if data.rsi_values[i] > data.rsi_values[i - 1] && data.rsi_values[i] > data.rsi_values[i + 1] {
                    rsi_peaks.push((i, data.rsi_values[i]));
                }
            }
            // Bearish divergence: higher price peak + lower RSI peak
            if price_peaks.len() >= 2 && rsi_peaks.len() >= 2 {
                let pp = &price_peaks[price_peaks.len() - 2..];
                let rp = &rsi_peaks[rsi_peaks.len() - 2..];
                if pp[1].1 > pp[0].1 && rp[1].1 < rp[0].1 {
                    rsi_divergence = Some("bearish");
                    data.signals.push(("RSI Bearish Divergence", 3, false));
                } else if pp[1].1 < pp[0].1 && rp[1].1 > rp[0].1 {
                    // Bullish: lower price trough + higher RSI trough (using peaks as proxy for troughs inverted)
                    rsi_divergence = Some("bullish");
                    data.signals.push(("RSI Bullish Divergence", 3, true));
                }
            }
        }

        // --- Enhanced Signal 2: MACD Divergence ---
        let mut macd_divergence: Option<&str> = None;
        if data.macd_result.histogram.len() >= 20 && closes.len() >= 20 {
            let hist = &data.macd_result.histogram;
            let n = hist.len();
            let price_tail = &closes[closes.len().saturating_sub(n)..];
            let mut price_peaks: Vec<(usize, f64)> = Vec::new();
            let mut hist_peaks: Vec<(usize, f64)> = Vec::new();
            let window = n.min(20);
            let start = n.saturating_sub(window);
            for i in (start + 1)..(n - 1) {
                if price_tail.len() > i + 1 && price_tail[i] > price_tail[i - 1] && price_tail[i] > price_tail[i + 1] {
                    price_peaks.push((i, price_tail[i]));
                }
                if hist[i] > hist[i - 1] && hist[i] > hist[i + 1] {
                    hist_peaks.push((i, hist[i]));
                }
            }
            if price_peaks.len() >= 2 && hist_peaks.len() >= 2 {
                let pp = &price_peaks[price_peaks.len() - 2..];
                let hp = &hist_peaks[hist_peaks.len() - 2..];
                if pp[1].1 > pp[0].1 && hp[1].1 < hp[0].1 {
                    macd_divergence = Some("bearish");
                    data.signals.push(("MACD Bearish Divergence", 3, false));
                } else if pp[1].1 < pp[0].1 && hp[1].1 > hp[0].1 {
                    macd_divergence = Some("bullish");
                    data.signals.push(("MACD Bullish Divergence", 3, true));
                }
            }
        }

        // --- Enhanced Signal 3a: Volume Confirmation for MA Crosses ---
        // If a golden/death cross happened, confirm with above-average volume
        {
            let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();
            let vol_sma = sma(&volumes, 20);
            if let (Some(&last_vol), Some(&last_vol_sma)) = (volumes.last(), vol_sma.last()) {
                let high_volume = last_vol > last_vol_sma * 1.2;
                // Check if golden/death cross just fired (in the signals we already built)
                let has_golden = data.signals.iter().any(|(name, _, _)| *name == "Golden Cross");
                let has_death = data.signals.iter().any(|(name, _, _)| *name == "Death Cross");
                if has_golden && high_volume {
                    data.signals.push(("Volume Confirms Golden Cross", 2, true));
                } else if has_death && high_volume {
                    data.signals.push(("Volume Confirms Death Cross", 2, false));
                } else if (has_golden || has_death) && !high_volume {
                    data.signals.push(("MA Cross on Low Volume (Weak)", 1, false));
                }
            }
        }

        // --- Enhanced Signal 3b: OBV Confirmation ---
        let obv_values = obv(bars);
        let mut obv_trend_str: Option<&str> = None;
        if obv_values.len() >= 20 {
            let obv_sma = sma(&obv_values, 20);
            if let (Some(&last_obv), Some(&last_obv_sma)) = (obv_values.last(), obv_sma.last()) {
                let obv_rising = last_obv > last_obv_sma;
                let price_rising = if closes.len() >= 20 {
                    closes.last().unwrap() > &closes[closes.len() - 20]
                } else {
                    false
                };
                if obv_rising == price_rising {
                    obv_trend_str = Some("confirming");
                    data.signals.push(("OBV Confirms Trend", 2, price_rising));
                } else {
                    obv_trend_str = Some("diverging");
                    // OBV divergence is a warning — bearish if price rising but OBV falling
                    data.signals.push(("OBV Divergence Warning", 2, !price_rising));
                }
            }
        }

        // --- Enhanced Signal 4: Volume Spike ---
        let volume_sma = sma(&volumes, 20);
        let volume_ratio = if let (Some(&last_vol), Some(&last_vol_sma)) = (volumes.last(), volume_sma.last()) {
            if last_vol_sma > 0.0 { last_vol / last_vol_sma } else { 1.0 }
        } else {
            1.0
        };
        if volume_ratio > 2.0 {
            let bar_closed_up = if bars.len() >= 2 {
                bars.last().unwrap().close > bars[bars.len() - 2].close
            } else {
                true
            };
            if bar_closed_up {
                data.signals.push(("Volume Spike (Bullish)", 2, true));
            } else {
                data.signals.push(("Volume Spike (Bearish)", 2, false));
            }
        }

        // --- Enhanced Signal 6: Oversold Mean-Reversion Opportunity ---
        if let Some(&last_rsi) = data.rsi_values.last() {
            if last_rsi < 40.0 && closes.len() >= 20 {
                let return_20d = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;
                if return_20d < -8.0 {
                    // Check if any of the last 3 bars show recovery
                    let recent_bars = &bars[bars.len().saturating_sub(3)..];
                    let has_recovery = recent_bars.windows(2).any(|w| w.len() == 2 && w[1].close > w[0].close)
                        || (bars.len() >= 4 && bars[bars.len() - 1].close > bars[bars.len() - 2].close);
                    if has_recovery {
                        data.signals.push(("Oversold Bounce Setup", 3, true));
                    } else {
                        data.signals.push(("Deep Oversold", 2, true));
                    }
                }
            }
        }

        // --- Enhanced Signal 5: ATR Volatility ---
        let atr_values = atr(bars, 14);
        let atr_sma = sma(&atr_values, 20);
        let atr_ratio = if let (Some(&last_atr), Some(&last_atr_sma)) = (atr_values.last(), atr_sma.last()) {
            if last_atr_sma > 0.0 { last_atr / last_atr_sma } else { 1.0 }
        } else {
            1.0
        };
        if atr_ratio > 1.5 {
            data.signals.push(("Volatility Expanding", 1, false));
        } else if atr_ratio < 0.6 {
            data.signals.push(("Volatility Contracting", 1, true));
        }

        // --- Overextension: Price Distance from 50-SMA ---
        if !data.sma_50.is_empty() {
            let current_price = *closes.last().unwrap();
            let last_sma_50 = *data.sma_50.last().unwrap();
            if last_sma_50 > 0.0 {
                let distance_pct = (current_price - last_sma_50) / last_sma_50 * 100.0;
                if distance_pct > 25.0 {
                    data.signals.push(("Price Severely Extended Above 50-SMA", 3, false));
                } else if distance_pct > 15.0 {
                    data.signals.push(("Price Extended Above 50-SMA", 2, false));
                } else if distance_pct < -25.0 {
                    data.signals.push(("Price Severely Extended Below 50-SMA", 3, true));
                } else if distance_pct < -15.0 {
                    data.signals.push(("Price Extended Below 50-SMA", 2, true));
                }
            }
        }

        // --- Overbought Exhaustion Setup (mirror of Oversold Bounce) ---
        if let Some(&last_rsi) = data.rsi_values.last() {
            if last_rsi > 60.0 && closes.len() >= 20 {
                let return_20d = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;
                if return_20d > 8.0 {
                    let recent_bars = &bars[bars.len().saturating_sub(3)..];
                    let selling_bars = recent_bars.iter().filter(|b| b.close < b.open).count();
                    if selling_bars >= 2 {
                        data.signals.push(("Overbought Pullback Setup", 3, false));
                    }
                }
            }
        }

        // --- Rate of Change Deceleration ---
        if closes.len() >= 20 {
            let roc_10 = if closes.len() >= 10 {
                (closes[closes.len() - 1] - closes[closes.len() - 10]) / closes[closes.len() - 10] * 100.0
            } else {
                0.0
            };
            let roc_20 = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;

            if roc_20 > 10.0 && roc_10 < roc_20 * 0.4 {
                data.signals.push(("Momentum Decelerating", 2, false));
            } else if roc_20 < -10.0 && roc_10 > roc_20 * 0.4 {
                data.signals.push(("Selling Pressure Easing", 2, true));
            }
        }

        // --- Recency: 5-Day Short-Term Momentum ---
        if closes.len() >= 6 {
            let five_day_return = (closes[closes.len() - 1] - closes[closes.len() - 6]) / closes[closes.len() - 6] * 100.0;
            if five_day_return > 5.0 {
                data.signals.push(("Strong Recent Buying", 2, true));
            } else if five_day_return > 2.0 {
                data.signals.push(("Recent Upward Pressure", 1, true));
            } else if five_day_return < -5.0 {
                data.signals.push(("Strong Recent Selling", 2, false));
            } else if five_day_return < -2.0 {
                data.signals.push(("Recent Downward Pressure", 1, false));
            }
        }

        // Calculate overall signal
        let mut total_score = 0;
        let mut total_weight = 0;
        for (_, weight, bullish) in &data.signals {
            total_weight += weight;
            total_score += if *bullish { *weight } else { -weight };
        }

        let normalized_score = if total_weight > 0 {
            (total_score as f64 / total_weight as f64) * 100.0
        } else {
            0.0
        };

        let signal = SignalStrength::from_score(normalized_score as i32);
        let confidence = (total_weight as f64 / 20.0).min(1.0);

        let reason = data.signals
            .iter()
            .map(|(name, _, bullish)| {
                format!("{} {}", if *bullish { "+" } else { "-" }, name)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let extra_metrics = json!({
            "obv_trend": obv_trend_str,
            "volume_ratio": volume_ratio,
            "atr_ratio": atr_ratio,
            "rsi_divergence": rsi_divergence,
            "macd_divergence": macd_divergence,
        });

        let metrics = self.build_metrics(&data, Some(extra_metrics));

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason,
            metrics,
        })
    }
}

#[async_trait]
impl TechnicalAnalyzer for TechnicalAnalysisEngine {
    async fn analyze(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError> {
        self.analyze_sync(symbol, bars)
    }
}

impl Default for TechnicalAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
