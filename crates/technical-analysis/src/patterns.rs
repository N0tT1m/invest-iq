use analysis_core::Bar;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CandlestickPattern {
    Doji,
    Hammer,
    InvertedHammer,
    ShootingStar,
    Engulfing,
    Piercing,
    DarkCloudCover,
    MorningStar,
    EveningStar,
    ThreeWhiteSoldiers,
    ThreeBlackCrows,
}

#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern: CandlestickPattern,
    pub index: usize,
    pub strength: f64, // 0.0 to 1.0
    pub bullish: bool,
}

/// Detect if a candle is a doji
fn is_doji(bar: &Bar) -> bool {
    let body = (bar.close - bar.open).abs();
    let range = bar.high - bar.low;
    range > 0.0 && body / range < 0.1
}

/// Detect hammer pattern
fn is_hammer(bar: &Bar) -> Option<PatternMatch> {
    let body = (bar.close - bar.open).abs();
    let range = bar.high - bar.low;
    let lower_shadow = bar.open.min(bar.close) - bar.low;
    let upper_shadow = bar.high - bar.open.max(bar.close);

    if range == 0.0 {
        return None;
    }

    // Hammer: small body, long lower shadow, little/no upper shadow
    if body / range < 0.3 && lower_shadow > 2.0 * body && upper_shadow < body * 0.5 {
        let strength = (lower_shadow / body).min(5.0) / 5.0;
        return Some(PatternMatch {
            pattern: CandlestickPattern::Hammer,
            index: 0,
            strength,
            bullish: true,
        });
    }

    None
}

/// Detect inverted hammer
fn is_inverted_hammer(bar: &Bar) -> Option<PatternMatch> {
    let body = (bar.close - bar.open).abs();
    let range = bar.high - bar.low;
    let lower_shadow = bar.open.min(bar.close) - bar.low;
    let upper_shadow = bar.high - bar.open.max(bar.close);

    if range == 0.0 {
        return None;
    }

    // Inverted hammer: small body, long upper shadow, little/no lower shadow
    if body / range < 0.3 && upper_shadow > 2.0 * body && lower_shadow < body * 0.5 {
        let strength = (upper_shadow / body).min(5.0) / 5.0;
        return Some(PatternMatch {
            pattern: CandlestickPattern::InvertedHammer,
            index: 0,
            strength,
            bullish: true,
        });
    }

    None
}

/// Detect shooting star
fn is_shooting_star(bar: &Bar) -> Option<PatternMatch> {
    let body = (bar.close - bar.open).abs();
    let range = bar.high - bar.low;
    let lower_shadow = bar.open.min(bar.close) - bar.low;
    let upper_shadow = bar.high - bar.open.max(bar.close);

    if range == 0.0 {
        return None;
    }

    // Shooting star: small body at bottom, long upper shadow
    if body / range < 0.3 && upper_shadow > 2.0 * body && lower_shadow < body * 0.5 {
        let strength = (upper_shadow / body).min(5.0) / 5.0;
        return Some(PatternMatch {
            pattern: CandlestickPattern::ShootingStar,
            index: 0,
            strength,
            bullish: false,
        });
    }

    None
}

/// Detect engulfing pattern (requires 2 bars)
fn is_engulfing(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 2 {
        return None;
    }

    let prev = &bars[bars.len() - 2];
    let curr = &bars[bars.len() - 1];

    let prev_bullish = prev.close > prev.open;
    let curr_bullish = curr.close > curr.open;

    // Bullish engulfing: prev bearish, curr bullish and engulfs prev
    if !prev_bullish && curr_bullish {
        if curr.open <= prev.close && curr.close >= prev.open {
            let body_size = (curr.close - curr.open) / (prev.open - prev.close);
            return Some(PatternMatch {
                pattern: CandlestickPattern::Engulfing,
                index: bars.len() - 1,
                strength: body_size.min(2.0) / 2.0,
                bullish: true,
            });
        }
    }

    // Bearish engulfing: prev bullish, curr bearish and engulfs prev
    if prev_bullish && !curr_bullish {
        if curr.open >= prev.close && curr.close <= prev.open {
            let body_size = (curr.open - curr.close) / (prev.close - prev.open);
            return Some(PatternMatch {
                pattern: CandlestickPattern::Engulfing,
                index: bars.len() - 1,
                strength: body_size.min(2.0) / 2.0,
                bullish: false,
            });
        }
    }

    None
}

/// Detect three white soldiers (requires 3 bars)
fn is_three_white_soldiers(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 3 {
        return None;
    }

    let last_3 = &bars[bars.len() - 3..];

    // All three must be bullish
    if last_3.iter().all(|b| b.close > b.open) {
        // Each candle should close higher than the previous
        if last_3[1].close > last_3[0].close && last_3[2].close > last_3[1].close {
            // Each candle should open within the body of the previous
            if last_3[1].open > last_3[0].open
                && last_3[1].open < last_3[0].close
                && last_3[2].open > last_3[1].open
                && last_3[2].open < last_3[1].close
            {
                return Some(PatternMatch {
                    pattern: CandlestickPattern::ThreeWhiteSoldiers,
                    index: bars.len() - 1,
                    strength: 0.8,
                    bullish: true,
                });
            }
        }
    }

    None
}

/// Detect piercing pattern (requires 2 bars) - bullish reversal
fn is_piercing(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 2 {
        return None;
    }

    let prev = &bars[bars.len() - 2];
    let curr = &bars[bars.len() - 1];

    let prev_bearish = prev.close < prev.open;
    let curr_bullish = curr.close > curr.open;

    // Piercing: prev bearish, curr opens below prev low, closes above midpoint of prev body
    if prev_bearish && curr_bullish {
        let prev_midpoint = (prev.open + prev.close) / 2.0;
        if curr.open < prev.low && curr.close > prev_midpoint && curr.close < prev.open {
            let penetration = (curr.close - prev.close) / (prev.open - prev.close);
            return Some(PatternMatch {
                pattern: CandlestickPattern::Piercing,
                index: bars.len() - 1,
                strength: penetration.min(1.0),
                bullish: true,
            });
        }
    }

    None
}

/// Detect dark cloud cover (requires 2 bars) - bearish reversal
fn is_dark_cloud_cover(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 2 {
        return None;
    }

    let prev = &bars[bars.len() - 2];
    let curr = &bars[bars.len() - 1];

    let prev_bullish = prev.close > prev.open;
    let curr_bearish = curr.close < curr.open;

    // Dark cloud: prev bullish, curr opens above prev high, closes below midpoint of prev body
    if prev_bullish && curr_bearish {
        let prev_midpoint = (prev.open + prev.close) / 2.0;
        if curr.open > prev.high && curr.close < prev_midpoint && curr.close > prev.open {
            let penetration = (prev.close - curr.close) / (prev.close - prev.open);
            return Some(PatternMatch {
                pattern: CandlestickPattern::DarkCloudCover,
                index: bars.len() - 1,
                strength: penetration.min(1.0),
                bullish: false,
            });
        }
    }

    None
}

/// Detect morning star (requires 3 bars) - bullish reversal
fn is_morning_star(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 3 {
        return None;
    }

    let first = &bars[bars.len() - 3];
    let star = &bars[bars.len() - 2];
    let third = &bars[bars.len() - 1];

    let first_bearish = first.close < first.open;
    let first_body = (first.open - first.close).abs();
    let star_body = (star.close - star.open).abs();
    let first_range = first.high - first.low;
    let third_bullish = third.close > third.open;

    // Morning star: large bearish, small-body star, large bullish closing above first bar midpoint
    if first_bearish && third_bullish && first_range > 0.0 {
        let first_midpoint = (first.open + first.close) / 2.0;
        if star_body < first_body * 0.3 && third.close > first_midpoint {
            return Some(PatternMatch {
                pattern: CandlestickPattern::MorningStar,
                index: bars.len() - 1,
                strength: 0.8,
                bullish: true,
            });
        }
    }

    None
}

/// Detect evening star (requires 3 bars) - bearish reversal
fn is_evening_star(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 3 {
        return None;
    }

    let first = &bars[bars.len() - 3];
    let star = &bars[bars.len() - 2];
    let third = &bars[bars.len() - 1];

    let first_bullish = first.close > first.open;
    let first_body = (first.close - first.open).abs();
    let star_body = (star.close - star.open).abs();
    let first_range = first.high - first.low;
    let third_bearish = third.close < third.open;

    // Evening star: large bullish, small-body star, large bearish closing below first bar midpoint
    if first_bullish && third_bearish && first_range > 0.0 {
        let first_midpoint = (first.open + first.close) / 2.0;
        if star_body < first_body * 0.3 && third.close < first_midpoint {
            return Some(PatternMatch {
                pattern: CandlestickPattern::EveningStar,
                index: bars.len() - 1,
                strength: 0.8,
                bullish: false,
            });
        }
    }

    None
}

/// Detect three black crows (requires 3 bars)
fn is_three_black_crows(bars: &[Bar]) -> Option<PatternMatch> {
    if bars.len() < 3 {
        return None;
    }

    let last_3 = &bars[bars.len() - 3..];

    // All three must be bearish
    if last_3.iter().all(|b| b.close < b.open) {
        // Each candle should close lower than the previous
        if last_3[1].close < last_3[0].close && last_3[2].close < last_3[1].close {
            // Each candle should open within the body of the previous
            if last_3[1].open < last_3[0].open
                && last_3[1].open > last_3[0].close
                && last_3[2].open < last_3[1].open
                && last_3[2].open > last_3[1].close
            {
                return Some(PatternMatch {
                    pattern: CandlestickPattern::ThreeBlackCrows,
                    index: bars.len() - 1,
                    strength: 0.8,
                    bullish: false,
                });
            }
        }
    }

    None
}

/// Detect all patterns in a set of bars
pub fn detect_patterns(bars: &[Bar]) -> Vec<PatternMatch> {
    let mut patterns = Vec::new();

    if bars.is_empty() {
        return patterns;
    }

    // Single candle patterns on the last bar
    let last = bars.last().unwrap();
    if is_doji(last) {
        patterns.push(PatternMatch {
            pattern: CandlestickPattern::Doji,
            index: bars.len() - 1,
            strength: 0.5,
            bullish: false,
        });
    }

    if let Some(p) = is_hammer(last) {
        patterns.push(PatternMatch { index: bars.len() - 1, ..p });
    }

    if let Some(p) = is_inverted_hammer(last) {
        patterns.push(PatternMatch { index: bars.len() - 1, ..p });
    }

    if let Some(p) = is_shooting_star(last) {
        patterns.push(PatternMatch { index: bars.len() - 1, ..p });
    }

    // Multi-candle patterns
    if let Some(p) = is_engulfing(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_three_white_soldiers(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_three_black_crows(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_piercing(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_dark_cloud_cover(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_morning_star(bars) {
        patterns.push(p);
    }

    if let Some(p) = is_evening_star(bars) {
        patterns.push(p);
    }

    patterns
}

/// Detect trend direction using highs and lows
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trend {
    Uptrend,
    Downtrend,
    Sideways,
}

pub fn detect_trend(bars: &[Bar], lookback: usize) -> Trend {
    if bars.len() < lookback {
        return Trend::Sideways;
    }

    let recent = &bars[bars.len() - lookback..];

    let highs: Vec<f64> = recent.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = recent.iter().map(|b| b.low).collect();

    // Simple linear regression to determine trend
    let n = highs.len() as f64;
    let x_sum: f64 = (0..highs.len()).map(|i| i as f64).sum();
    let y_sum_high: f64 = highs.iter().sum();
    let y_sum_low: f64 = lows.iter().sum();

    let xy_sum_high: f64 = highs.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
    let xy_sum_low: f64 = lows.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();

    let x_squared_sum: f64 = (0..highs.len()).map(|i| (i as f64).powi(2)).sum();

    let slope_high = (n * xy_sum_high - x_sum * y_sum_high) / (n * x_squared_sum - x_sum.powi(2));
    let slope_low = (n * xy_sum_low - x_sum * y_sum_low) / (n * x_squared_sum - x_sum.powi(2));

    let avg_slope = (slope_high + slope_low) / 2.0;
    let price_range = recent.iter().map(|b| b.high - b.low).sum::<f64>() / n;

    // If slope is significant relative to price range, determine trend
    if avg_slope > price_range * 0.1 {
        Trend::Uptrend
    } else if avg_slope < -price_range * 0.1 {
        Trend::Downtrend
    } else {
        Trend::Sideways
    }
}
