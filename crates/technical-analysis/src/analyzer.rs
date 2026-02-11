use analysis_core::{adaptive, AnalysisError, AnalysisResult, Bar, SignalStrength, TechnicalAnalyzer};
use async_trait::async_trait;
use chrono::Utc;
use rayon;
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

    /// Build the core set of signals shared by both analyze_sync and analyze_enhanced.
    /// Uses rayon::join to compute independent indicator groups in parallel.
    fn build_signals(&self, bars: &[Bar]) -> Result<SignalData, AnalysisError> {
        if bars.len() < 50 {
            return Err(AnalysisError::InsufficientData(
                "Need at least 50 bars for technical analysis".to_string(),
            ));
        }

        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();

        // Compute independent indicator groups in parallel using rayon::join
        // Group 1: RSI + MACD + ADX
        // Group 2: SMA + Bollinger Bands + Stochastic
        // Group 3: Patterns + Support/Resistance + Trend
        let (group1, group2) = rayon::join(
            || {
                rayon::join(
                    || {
                        let rsi_values = rsi(&closes, 14);
                        let macd_result = macd(&closes, 12, 26, 9);
                        (rsi_values, macd_result)
                    },
                    || adx(bars, 14),
                )
            },
            || {
                rayon::join(
                    || {
                        let sma_20 = sma(&closes, 20);
                        let sma_50 = sma(&closes, 50);
                        let bb = bollinger_bands(&closes, 20, 2.0);
                        (sma_20, sma_50, bb)
                    },
                    || stochastic(bars, 14, 3),
                )
            },
        );
        let ((rsi_values, macd_result), adx_result) = group1;
        let ((sma_20, sma_50, bb), stoch) = group2;

        // These are fast O(n) scans, run in parallel too
        let (patterns, (sr, trend)) = rayon::join(
            || detect_patterns(bars),
            || {
                let sr = support_resistance(bars, 30.min(bars.len()));
                let trend = detect_trend(bars, 20);
                (sr, trend)
            },
        );

        // Now generate signals from computed indicators (fast, sequential)
        let mut signals: Vec<(&'static str, i32, bool)> = Vec::new();

        // RSI Analysis
        if let Some(&last_rsi) = rsi_values.last() {
            let rsi_pct = adaptive::percentile_rank(last_rsi, &rsi_values);
            if rsi_pct < 0.10 {
                signals.push(("RSI Deeply Oversold", 3, true));
            } else if rsi_pct < 0.20 {
                signals.push(("RSI Oversold", 2, true));
            } else if rsi_pct > 0.90 {
                signals.push(("RSI Overbought", 2, false));
            }
        }

        // MACD Analysis
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
        if !stoch.k.is_empty() {
            let last_k = stoch.k.last().unwrap();
            let stoch_pct = adaptive::percentile_rank(*last_k, &stoch.k);
            if stoch_pct < 0.15 {
                signals.push(("Stochastic Oversold", 2, true));
            } else if stoch_pct > 0.85 {
                signals.push(("Stochastic Overbought", 2, false));
            }
        }

        // Pattern Detection
        for pattern in &patterns {
            let weight = (pattern.strength * 3.0) as i32;
            signals.push((pattern_name(&pattern.pattern), weight, pattern.bullish));
        }

        // ADX Trend Strength
        let last_adx = adx_result.adx.last().copied();
        if let Some(adx_val) = last_adx {
            let adx_pct = adaptive::percentile_rank(adx_val, &adx_result.adx);
            if adx_pct > 0.75 {
                let last_pdi = adx_result.plus_di.last().copied().unwrap_or(0.0);
                let last_mdi = adx_result.minus_di.last().copied().unwrap_or(0.0);
                if last_pdi > last_mdi {
                    signals.push(("Strong Bullish Trend (ADX)", 3, true));
                } else {
                    signals.push(("Strong Bearish Trend (ADX)", 3, false));
                }
            }
        }

        // Support / Resistance proximity
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

    /// Enhanced technical analysis with frontier-level signals
    pub fn analyze_enhanced(&self, symbol: &str, bars: &[Bar], spy_bars: Option<&[Bar]>) -> Result<AnalysisResult, AnalysisError> {
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
        // If a golden/death cross happened, confirm with above-average volume (adaptive z-score)
        {
            let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();
            let vol_sma = sma(&volumes, 20);
            if let (Some(&last_vol), Some(&last_vol_sma)) = (volumes.last(), vol_sma.last()) {
                let volume_ratio = if last_vol_sma > 0.0 { last_vol / last_vol_sma } else { 1.0 };
                let vol_z = adaptive::z_score_of(volume_ratio, &volumes.iter().enumerate()
                    .filter_map(|(i, &v)| {
                        if i >= 20 && i < volumes.len() {
                            let sma_val = vol_sma.get(i - 20)?;
                            Some(if *sma_val > 0.0 { v / sma_val } else { 1.0 })
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>());
                let high_volume = vol_z > 0.5; // z-score > 0.5 = above average
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

        // --- Enhanced Signal 4: Volume Spike (adaptive z-score)---
        let volume_sma = sma(&volumes, 20);
        let volume_ratio = if let (Some(&last_vol), Some(&last_vol_sma)) = (volumes.last(), volume_sma.last()) {
            if last_vol_sma > 0.0 { last_vol / last_vol_sma } else { 1.0 }
        } else {
            1.0
        };
        let vol_z = adaptive::z_score_of(*volumes.last().unwrap(), &volumes);
        if vol_z > 2.0 {
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

        // --- Enhanced Signal 6: Oversold Mean-Reversion Opportunity (adaptive z-score) ---
        if let Some(&last_rsi) = data.rsi_values.last() {
            let rsi_pct = adaptive::percentile_rank(last_rsi, &data.rsi_values);
            if rsi_pct < 0.40 && closes.len() >= 20 {
                // Compute all 20-day rolling returns
                let returns_20d: Vec<f64> = (20..closes.len())
                    .map(|i| (closes[i] - closes[i - 20]) / closes[i - 20] * 100.0)
                    .collect();
                let current_return_20d = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;
                let return_z = adaptive::z_score_of(current_return_20d, &returns_20d);
                if return_z < -2.0 {
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

        // --- Enhanced Signal 5: ATR Volatility (adaptive percentile-based) ---
        let atr_values = atr(bars, 14);
        let atr_sma = sma(&atr_values, 20);
        let atr_ratio = if let (Some(&last_atr), Some(&last_atr_sma)) = (atr_values.last(), atr_sma.last()) {
            if last_atr_sma > 0.0 { last_atr / last_atr_sma } else { 1.0 }
        } else {
            1.0
        };
        // Compute historical ATR ratios
        let atr_ratios: Vec<f64> = if atr_sma.len() > 0 {
            atr_values.iter().zip(atr_sma.iter())
                .map(|(atr, sma)| if *sma > 0.0 { atr / sma } else { 1.0 })
                .collect()
        } else {
            vec![1.0]
        };
        let atr_pct = adaptive::percentile_rank(atr_ratio, &atr_ratios);
        if atr_pct > 0.85 {
            data.signals.push(("Volatility Expanding", 1, false));
        } else if atr_pct < 0.15 {
            data.signals.push(("Volatility Contracting", 1, true));
        }

        // --- Overextension: Price Distance from 50-SMA (adaptive z-score) ---
        if !data.sma_50.is_empty() && closes.len() >= 50 {
            let current_price = *closes.last().unwrap();
            let last_sma_50 = *data.sma_50.last().unwrap();
            if last_sma_50 > 0.0 {
                // Compute historical distances for all bars with SMA-50
                let distances: Vec<f64> = closes.iter().zip(data.sma_50.iter())
                    .filter_map(|(c, sma)| {
                        if *sma > 0.0 {
                            Some((c - sma) / sma * 100.0)
                        } else {
                            None
                        }
                    })
                    .collect();
                let distance_pct = (current_price - last_sma_50) / last_sma_50 * 100.0;
                let dist_z = adaptive::z_score_of(distance_pct, &distances);
                if dist_z > 2.0 {
                    data.signals.push(("Price Severely Extended Above 50-SMA", 3, false));
                } else if dist_z > 1.5 {
                    data.signals.push(("Price Extended Above 50-SMA", 2, false));
                } else if dist_z < -2.0 {
                    data.signals.push(("Price Severely Extended Below 50-SMA", 3, true));
                } else if dist_z < -1.5 {
                    data.signals.push(("Price Extended Below 50-SMA", 2, true));
                }
            }
        }

        // --- Overbought Exhaustion Setup (adaptive z-score, mirror of Oversold Bounce) ---
        if let Some(&last_rsi) = data.rsi_values.last() {
            let rsi_pct = adaptive::percentile_rank(last_rsi, &data.rsi_values);
            if rsi_pct > 0.60 && closes.len() >= 20 {
                // Compute all 20-day rolling returns
                let returns_20d: Vec<f64> = (20..closes.len())
                    .map(|i| (closes[i] - closes[i - 20]) / closes[i - 20] * 100.0)
                    .collect();
                let current_return_20d = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;
                let return_z = adaptive::z_score_of(current_return_20d, &returns_20d);
                if return_z > 2.0 {
                    let recent_bars = &bars[bars.len().saturating_sub(3)..];
                    let selling_bars = recent_bars.iter().filter(|b| b.close < b.open).count();
                    if selling_bars >= 2 {
                        data.signals.push(("Overbought Pullback Setup", 3, false));
                    }
                }
            }
        }

        // --- Rate of Change Deceleration (adaptive z-score) ---
        if closes.len() >= 20 {
            let roc_10 = if closes.len() >= 10 {
                (closes[closes.len() - 1] - closes[closes.len() - 10]) / closes[closes.len() - 10] * 100.0
            } else {
                0.0
            };
            let roc_20 = (closes[closes.len() - 1] - closes[closes.len() - 20]) / closes[closes.len() - 20] * 100.0;

            // Compute all 20-day RoC values for z-score
            let roc_20_values: Vec<f64> = (20..closes.len())
                .map(|i| (closes[i] - closes[i - 20]) / closes[i - 20] * 100.0)
                .collect();
            let roc_z = adaptive::z_score_of(roc_20, &roc_20_values);

            if roc_z > 2.0 && roc_10 < roc_20 * 0.4 {
                data.signals.push(("Momentum Decelerating", 2, false));
            } else if roc_z < -2.0 && roc_10 > roc_20 * 0.4 {
                data.signals.push(("Selling Pressure Easing", 2, true));
            }
        }

        // --- Recency: 5-Day Short-Term Momentum (adaptive percentile-based) ---
        if closes.len() >= 6 {
            let five_day_return = (closes[closes.len() - 1] - closes[closes.len() - 6]) / closes[closes.len() - 6] * 100.0;
            // Compute all 5-day rolling returns
            let returns_5d: Vec<f64> = (6..closes.len())
                .map(|i| (closes[i] - closes[i - 5]) / closes[i - 5] * 100.0)
                .collect();
            let return_pct = adaptive::percentile_rank(five_day_return, &returns_5d);
            if return_pct > 0.90 {
                data.signals.push(("Strong Recent Buying", 2, true));
            } else if return_pct > 0.75 {
                data.signals.push(("Recent Upward Pressure", 1, true));
            } else if return_pct < 0.10 {
                data.signals.push(("Strong Recent Selling", 2, false));
            } else if return_pct < 0.25 {
                data.signals.push(("Recent Downward Pressure", 1, false));
            }
        }

        // --- Regime-Adaptive RSI (percentile-based adaptive thresholds) ---
        // Remove static RSI signals and replace with trend-adaptive percentile thresholds
        if let Some(&last_rsi) = data.rsi_values.last() {
            // Compute adaptive percentile bounds from RSI distribution
            let mut rsi_sorted = data.rsi_values.clone();
            rsi_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let (p_oversold, p_overbought) = match data.trend {
                Trend::Uptrend => (0.05, 0.95),   // Shift bounds: less sensitive to oversold, more to overbought
                Trend::Downtrend => (0.05, 0.95), // Mirror: more sensitive to oversold
                Trend::Sideways => (0.10, 0.90),  // Standard bounds
            };

            let oversold_threshold = if !rsi_sorted.is_empty() {
                let idx = ((rsi_sorted.len() as f64 * p_oversold).floor() as usize).min(rsi_sorted.len() - 1);
                rsi_sorted[idx]
            } else {
                30.0
            };

            let overbought_threshold = if !rsi_sorted.is_empty() {
                let idx = ((rsi_sorted.len() as f64 * p_overbought).floor() as usize).min(rsi_sorted.len() - 1);
                rsi_sorted[idx]
            } else {
                70.0
            };

            // Remove any static RSI signals that build_signals added
            data.signals.retain(|(name, _, _)| !name.contains("RSI Deeply Oversold") && !name.contains("RSI Oversold") && !name.contains("RSI Overbought"));

            let rsi_pct = adaptive::percentile_rank(last_rsi, &data.rsi_values);
            if rsi_pct < p_oversold / 2.0 {
                data.signals.push(("RSI Deeply Oversold (Adaptive)", 3, true));
            } else if last_rsi < oversold_threshold {
                data.signals.push(("RSI Oversold (Adaptive)", 2, true));
            } else if last_rsi > overbought_threshold {
                data.signals.push(("RSI Overbought (Adaptive)", 2, false));
            }
        }

        // --- Multi-Timeframe Confluence (Weekly) ---
        let mut weekly_trend_str: Option<&str> = None;
        let mut weekly_rsi_val: Option<f64> = None;
        if bars.len() >= 60 {
            let weekly_bars = resample_to_weekly(bars);
            if weekly_bars.len() >= 20 {
                let weekly_closes: Vec<f64> = weekly_bars.iter().map(|b| b.close).collect();
                let weekly_rsi = rsi(&weekly_closes, 14);
                weekly_rsi_val = weekly_rsi.last().copied();

                let w_trend = detect_trend(&weekly_bars, 10.min(weekly_bars.len()));
                weekly_trend_str = Some(match w_trend {
                    Trend::Uptrend => "uptrend",
                    Trend::Downtrend => "downtrend",
                    Trend::Sideways => "sideways",
                });

                // Confluence: weekly aligns with daily
                match (&data.trend, &w_trend) {
                    (Trend::Uptrend, Trend::Uptrend) => {
                        data.signals.push(("Weekly Confirms Uptrend", 3, true));
                    }
                    (Trend::Downtrend, Trend::Downtrend) => {
                        data.signals.push(("Weekly Confirms Downtrend", 3, false));
                    }
                    (Trend::Uptrend, Trend::Downtrend) | (Trend::Downtrend, Trend::Uptrend) => {
                        data.signals.push(("Weekly Contradicts Daily Trend", 2, match data.trend {
                            Trend::Uptrend => false, // weekly disagrees with daily bull
                            _ => true,               // weekly disagrees with daily bear
                        }));
                    }
                    _ => {}
                }
            }
        }

        // --- Relative Strength vs SPY ---
        let mut rs_rising: Option<bool> = None;
        if let Some(spy) = spy_bars {
            let spy_closes: Vec<f64> = spy.iter().map(|b| b.close).collect();
            let rs_line = relative_strength(closes, &spy_closes);
            if rs_line.len() >= 20 {
                let rs_sma = sma(&rs_line, 20);
                if let (Some(&last_rs), Some(&last_rs_sma)) = (rs_line.last(), rs_sma.last()) {
                    rs_rising = Some(last_rs > last_rs_sma);
                    if last_rs > last_rs_sma {
                        data.signals.push(("Outperforming Market (RS)", 2, true));
                        // Check for new RS highs
                        let rs_max = rs_line.iter().take(rs_line.len() - 1).cloned().fold(f64::NEG_INFINITY, f64::max);
                        if last_rs > rs_max {
                            data.signals.push(("New Relative Strength High", 2, true));
                        }
                    } else {
                        data.signals.push(("Underperforming Market (RS)", 2, false));
                    }
                }
            }
        }

        // --- Ichimoku Cloud Signals ---
        let mut ichimoku_signal: Option<&str> = None;
        if bars.len() >= 52 {
            let ichi = ichimoku(bars);
            let current_price = *closes.last().unwrap();
            // Price vs Cloud: use the last span_a and span_b values
            if let (Some(&span_a), Some(&span_b)) = (ichi.senkou_span_a.last(), ichi.senkou_span_b.last()) {
                let cloud_top = span_a.max(span_b);
                let cloud_bottom = span_a.min(span_b);
                if current_price > cloud_top {
                    data.signals.push(("Price Above Ichimoku Cloud", 2, true));
                    ichimoku_signal = Some("above_cloud");
                } else if current_price < cloud_bottom {
                    data.signals.push(("Price Below Ichimoku Cloud", 2, false));
                    ichimoku_signal = Some("below_cloud");
                } else {
                    ichimoku_signal = Some("in_cloud");
                }
                // Future cloud color
                if span_a > span_b {
                    data.signals.push(("Ichimoku Cloud Bullish (Green)", 1, true));
                } else {
                    data.signals.push(("Ichimoku Cloud Bearish (Red)", 1, false));
                }
            }
            // Tenkan/Kijun cross
            if ichi.tenkan_sen.len() >= 2 && ichi.kijun_sen.len() >= 2 {
                let n = ichi.tenkan_sen.len();
                let t_now = ichi.tenkan_sen[n - 1];
                let t_prev = ichi.tenkan_sen[n - 2];
                let k_now = ichi.kijun_sen[n - 1];
                let k_prev = ichi.kijun_sen[n - 2];
                if t_now > k_now && t_prev <= k_prev {
                    data.signals.push(("Tenkan-Kijun Bullish Cross", 3, true));
                } else if t_now < k_now && t_prev >= k_prev {
                    data.signals.push(("Tenkan-Kijun Bearish Cross", 3, false));
                }
            }
        }

        // --- Fibonacci Retracement Signals ---
        let mut fib_near_level: Option<&str> = None;
        if let Some(fib) = fibonacci_retracement(bars, 60.min(bars.len())) {
            let current_price = *closes.last().unwrap();
            let levels = [
                ("23.6%", fib.level_236), ("38.2%", fib.level_382),
                ("50.0%", fib.level_500), ("61.8%", fib.level_618),
                ("78.6%", fib.level_786),
            ];
            for (label, level) in &levels {
                let distance_pct = ((current_price - level) / current_price * 100.0).abs();
                if distance_pct < 1.5 {
                    let is_key_level = *label == "61.8%" || *label == "38.2%";
                    let weight = if is_key_level { 2 } else { 1 };
                    // Near a fib level can act as support (in uptrend) or resistance (in downtrend)
                    let bullish = match data.trend {
                        Trend::Uptrend => true,   // fib as support in uptrend
                        Trend::Downtrend => false, // fib as resistance in downtrend
                        Trend::Sideways => current_price > *level,
                    };
                    data.signals.push(("Near Fibonacci Level", weight, bullish));
                    fib_near_level = Some(label);
                    break;
                }
            }
        }

        // --- VWAP Signal ---
        let mut vwap_position: Option<&str> = None;
        {
            let vwap_values = vwap(bars);
            if let (Some(&last_vwap), Some(&current)) = (vwap_values.last(), closes.last()) {
                if last_vwap > 0.0 {
                    if current > last_vwap * 1.005 {
                        data.signals.push(("Price Above VWAP", 1, true));
                        vwap_position = Some("above");
                    } else if current < last_vwap * 0.995 {
                        data.signals.push(("Price Below VWAP", 1, false));
                        vwap_position = Some("below");
                    }
                }
            }
        }

        // --- Volume Profile / Supply-Demand Zones ---
        let mut vol_profile_support: Option<f64> = None;
        let mut vol_profile_resistance: Option<f64> = None;
        let mut accumulation_distribution: Option<&str> = None;
        let mut keltner_squeeze: Option<bool> = None;
        let mut near_pivot: Option<&str> = None;
        let mut market_struct_signal: Option<&str> = None;
        let mut divergence_quality: Option<&str> = None;
        let trend_strength_score: f64;

        if bars.len() >= 50 {
            // Volume-at-price: bucket prices and find highest-volume levels
            let price_min = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
            let price_max = bars.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
            let range = price_max - price_min;
            if range > 0.0 {
                let num_buckets = 20;
                let bucket_size = range / num_buckets as f64;
                let mut buckets = vec![0.0_f64; num_buckets];
                for bar in bars.iter() {
                    let mid = (bar.high + bar.low) / 2.0;
                    let idx = ((mid - price_min) / bucket_size).floor() as usize;
                    let idx = idx.min(num_buckets - 1);
                    buckets[idx] += bar.volume;
                }

                let current = closes.last().copied().unwrap_or(0.0);
                let current_bucket = ((current - price_min) / bucket_size).floor() as usize;
                let current_bucket = current_bucket.min(num_buckets - 1);

                // Find highest volume bucket below current price (support)
                if current_bucket > 0 {
                    let (sup_idx, _) = buckets[..current_bucket].iter().enumerate()
                        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .unwrap_or((0, &0.0));
                    let sup_price = price_min + (sup_idx as f64 + 0.5) * bucket_size;
                    vol_profile_support = Some(sup_price);
                    let dist_pct = (current - sup_price) / current * 100.0;
                    if dist_pct < 3.0 && dist_pct > 0.0 {
                        data.signals.push(("Near Volume Support", 2, true));
                    }
                }

                // Find highest volume bucket above current price (resistance)
                if current_bucket < num_buckets - 1 {
                    let (res_idx, _) = buckets[current_bucket + 1..].iter().enumerate()
                        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .unwrap_or((0, &0.0));
                    let res_price = price_min + ((current_bucket + 1 + res_idx) as f64 + 0.5) * bucket_size;
                    vol_profile_resistance = Some(res_price);
                    let dist_pct = (res_price - current) / current * 100.0;
                    if dist_pct < 3.0 && dist_pct > 0.0 {
                        data.signals.push(("Near Volume Resistance", 2, false));
                    }
                }
            }

            // Accumulation/Distribution: where does price close within the bar range?
            let recent_bars = &bars[bars.len().saturating_sub(20)..];
            let mut ad_score = 0.0_f64;
            for bar in recent_bars {
                let range = bar.high - bar.low;
                if range > 0.0 {
                    // CLV (Close Location Value): +1 = closed at high, -1 = closed at low
                    let clv = ((bar.close - bar.low) - (bar.high - bar.close)) / range;
                    ad_score += clv * bar.volume;
                }
            }
            if ad_score > 0.0 {
                accumulation_distribution = Some("accumulation");
                data.signals.push(("Accumulation Pattern", 2, true));
            } else if ad_score < 0.0 {
                accumulation_distribution = Some("distribution");
                data.signals.push(("Distribution Pattern", 2, false));
            }

            // --- Keltner Channels Squeeze ---
            if bars.len() >= 30 {
                let kc = keltner_channels(bars, 20, 10, 2.0);
                if !kc.upper.is_empty() && !data.bb.upper.is_empty() {
                    let kc_width = kc.upper.last().unwrap() - kc.lower.last().unwrap();
                    let bb_idx = data.bb.upper.len().saturating_sub(kc.upper.len());
                    if bb_idx < data.bb.upper.len() {
                        let bb_width = data.bb.upper[bb_idx] - data.bb.lower[bb_idx];
                        if bb_width < kc_width {
                            keltner_squeeze = Some(true);
                            data.signals.push(("Volatility Squeeze (Keltner/BB)", 2, true));
                        }
                    }
                }
            }

            // --- Pivot Point Proximity ---
            if let Some(pivots) = pivot_points(bars) {
                let current = *closes.last().unwrap();
                let tolerance = current * 0.005;
                if (current - pivots.pivot).abs() < tolerance {
                    near_pivot = Some("pivot");
                    data.signals.push(("Near Pivot Point", 1, true));
                } else if (current - pivots.r1).abs() < tolerance {
                    near_pivot = Some("r1");
                    data.signals.push(("Near R1 Resistance", 2, false));
                } else if (current - pivots.s1).abs() < tolerance {
                    near_pivot = Some("s1");
                    data.signals.push(("Near S1 Support", 2, true));
                }
            }

            // --- Market Structure Analysis ---
            if bars.len() >= 40 {
                let ms = market_structure(bars, 30);
                let uptrend_score = ms.higher_highs + ms.higher_lows;
                let downtrend_score = ms.lower_lows + ms.lower_highs;
                if uptrend_score > downtrend_score && uptrend_score >= 3 {
                    market_struct_signal = Some("bullish");
                    data.signals.push(("Bullish Market Structure", 2, true));
                } else if downtrend_score > uptrend_score && downtrend_score >= 3 {
                    market_struct_signal = Some("bearish");
                    data.signals.push(("Bearish Market Structure", 2, false));
                }
            }

            // --- Divergence Quality Scoring ---
            if rsi_divergence.is_some() || macd_divergence.is_some() {
                let vol_confirmation = if volumes.len() >= 20 {
                    let vol_sma = sma(&volumes, 10);
                    if let (Some(&last_vol), Some(&last_vol_sma)) = (volumes.last(), vol_sma.last()) {
                        last_vol > last_vol_sma * 1.2
                    } else { false }
                } else { false };
                let sr_confirmation = data.support.is_some() || data.resistance.is_some();
                if vol_confirmation && sr_confirmation {
                    divergence_quality = Some("high");
                    data.signals.push(("High-Quality Divergence", 3, rsi_divergence.is_some()));
                } else if vol_confirmation || sr_confirmation {
                    divergence_quality = Some("moderate");
                } else {
                    divergence_quality = Some("low");
                }
            }

        }

        // --- Trend Strength Composite (must be outside if block) ---
        trend_strength_score = {
            let mut score = 0.0;
            let mut components = 0.0;
            if let Some(adx_val) = data.adx_value {
                score += (adx_val / 50.0).min(1.0);
                components += 1.0;
            }
            if !data.sma_20.is_empty() && !data.sma_50.is_empty() {
                let sma_20 = *data.sma_20.last().unwrap();
                let sma_50 = *data.sma_50.last().unwrap();
                let current = *closes.last().unwrap();
                let alignment = if current > sma_20 && sma_20 > sma_50 { 1.0 }
                    else if current < sma_20 && sma_20 < sma_50 { 0.0 }
                    else { 0.5 };
                score += alignment;
                components += 1.0;
            }
            if market_struct_signal.is_some() {
                score += if market_struct_signal == Some("bullish") { 1.0 } else { 0.0 };
                components += 1.0;
            }
            if components > 0.0 { score / components } else { 0.5 }
        };

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
            "weekly_trend": weekly_trend_str,
            "weekly_rsi": weekly_rsi_val,
            "relative_strength_rising": rs_rising,
            "ichimoku_signal": ichimoku_signal,
            "fibonacci_near_level": fib_near_level,
            "vwap_position": vwap_position,
            "volume_profile_support": vol_profile_support,
            "volume_profile_resistance": vol_profile_resistance,
            "accumulation_distribution": accumulation_distribution,
            "keltner_squeeze": keltner_squeeze,
            "near_pivot": near_pivot,
            "market_structure": market_struct_signal,
            "divergence_quality": divergence_quality,
            "trend_strength_score": trend_strength_score,
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
