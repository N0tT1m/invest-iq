use analysis_core::Bar;

/// Return val if it is finite, otherwise return default.
#[inline]
pub fn finite_or(val: f64, default: f64) -> f64 {
    if val.is_finite() {
        val
    } else {
        default
    }
}

/// Simple Moving Average
pub fn sma(data: &[f64], period: usize) -> Vec<f64> {
    if period == 0 || data.len() < period {
        return vec![];
    }

    let mut result = Vec::with_capacity(data.len() - period + 1);
    for i in period - 1..data.len() {
        let sum: f64 = data[i + 1 - period..=i].iter().sum();
        result.push(finite_or(sum / period as f64, 0.0));
    }
    result
}

/// Exponential Moving Average
pub fn ema(data: &[f64], period: usize) -> Vec<f64> {
    if period == 0 || data.is_empty() {
        return vec![];
    }

    let mut result = Vec::with_capacity(data.len());
    let multiplier = 2.0 / (period as f64 + 1.0);

    // Not enough data for a full SMA seed -- return partial SMA
    if data.len() < period {
        let avg = data.iter().sum::<f64>() / data.len() as f64;
        return vec![finite_or(avg, 0.0)];
    }

    // Seed: SMA over the first `period` elements
    let sma_seed: f64 = data[..period].iter().sum::<f64>() / period as f64;
    let sma_seed = finite_or(sma_seed, 0.0);

    // Fill the first `period` slots with the SMA so the output length
    // matches the input length (callers like MACD and Keltner rely on this).
    for _ in 0..period {
        result.push(sma_seed);
    }

    // EMA smoothing starts at index `period` (the element right after the SMA window)
    for i in period..data.len() {
        let prev_ema = result[i - 1];
        let ema_val = (data[i] - prev_ema) * multiplier + prev_ema;
        result.push(finite_or(ema_val, prev_ema));
    }

    result
}

/// Relative Strength Index
pub fn rsi(data: &[f64], period: usize) -> Vec<f64> {
    if period == 0 || data.len() < period + 1 {
        return vec![];
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    for i in 1..data.len() {
        let change = data[i] - data[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(change.abs());
        }
    }

    let mut avg_gain = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss = losses[..period].iter().sum::<f64>() / period as f64;

    let mut rsi_values = Vec::with_capacity(data.len() - period);

    for i in period..gains.len() {
        avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;

        let rs = if avg_loss == 0.0 {
            100.0
        } else {
            avg_gain / avg_loss
        };

        let rsi = 100.0 - (100.0 / (1.0 + rs));
        rsi_values.push(finite_or(rsi, 50.0));
    }

    rsi_values
}

/// MACD (Moving Average Convergence Divergence)
pub struct MacdResult {
    pub macd_line: Vec<f64>,
    pub signal_line: Vec<f64>,
    pub histogram: Vec<f64>,
}

pub fn macd(
    data: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> MacdResult {
    if fast_period == 0 || slow_period == 0 || signal_period == 0 || slow_period < fast_period {
        return MacdResult {
            macd_line: vec![],
            signal_line: vec![],
            histogram: vec![],
        };
    }

    let ema_fast = ema(data, fast_period);
    let ema_slow = ema(data, slow_period);

    let offset = slow_period - fast_period;
    let mut macd_line = Vec::new();

    for i in offset..ema_fast.len() {
        macd_line.push(ema_fast[i] - ema_slow[i - offset]);
    }

    let signal_line = ema(&macd_line, signal_period);

    let mut histogram = Vec::new();
    let hist_offset = macd_line.len().saturating_sub(signal_line.len());
    for i in 0..signal_line.len() {
        histogram.push(macd_line[i + hist_offset] - signal_line[i]);
    }

    MacdResult {
        macd_line,
        signal_line,
        histogram,
    }
}

/// Bollinger Bands
pub struct BollingerBands {
    pub upper: Vec<f64>,
    pub middle: Vec<f64>,
    pub lower: Vec<f64>,
}

pub fn bollinger_bands(data: &[f64], period: usize, std_dev: f64) -> BollingerBands {
    if period == 0 || data.len() < period {
        return BollingerBands {
            upper: vec![],
            middle: vec![],
            lower: vec![],
        };
    }

    let middle = sma(data, period);
    let mut upper = Vec::with_capacity(middle.len());
    let mut lower = Vec::with_capacity(middle.len());

    for i in period - 1..data.len() {
        let slice = &data[i + 1 - period..=i];
        let mean = middle[i + 1 - period];
        let variance: f64 = slice.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / period as f64;
        let std = variance.sqrt();

        upper.push(finite_or(mean + std_dev * std, mean));
        lower.push(finite_or(mean - std_dev * std, mean));
    }

    BollingerBands {
        upper,
        middle,
        lower,
    }
}

/// Average True Range
pub fn atr(bars: &[Bar], period: usize) -> Vec<f64> {
    if period == 0 || bars.len() < period + 1 {
        return vec![];
    }

    let mut true_ranges = Vec::new();

    for i in 1..bars.len() {
        let high_low = bars[i].high - bars[i].low;
        let high_close = (bars[i].high - bars[i - 1].close).abs();
        let low_close = (bars[i].low - bars[i - 1].close).abs();

        let tr = high_low.max(high_close).max(low_close);
        true_ranges.push(tr);
    }

    let mut atr_values = Vec::new();
    let mut atr = true_ranges[..period].iter().sum::<f64>() / period as f64;
    atr = finite_or(atr, 0.0);
    atr_values.push(atr);

    for tr in &true_ranges[period..] {
        atr = (atr * (period - 1) as f64 + tr) / period as f64;
        atr_values.push(finite_or(atr, 0.0));
    }

    atr_values
}

/// Stochastic Oscillator
pub struct StochasticResult {
    pub k: Vec<f64>,
    pub d: Vec<f64>,
}

pub fn stochastic(bars: &[Bar], k_period: usize, d_period: usize) -> StochasticResult {
    if k_period == 0 || bars.len() < k_period {
        return StochasticResult {
            k: vec![],
            d: vec![],
        };
    }

    let mut k_values = Vec::new();

    for i in k_period - 1..bars.len() {
        let slice = &bars[i + 1 - k_period..=i];
        let highest = slice
            .iter()
            .map(|b| b.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let lowest = slice.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);

        let k = if highest == lowest {
            50.0
        } else {
            100.0 * (bars[i].close - lowest) / (highest - lowest)
        };

        k_values.push(finite_or(k, 50.0));
    }

    let d_values = sma(&k_values, d_period);

    StochasticResult {
        k: k_values,
        d: d_values,
    }
}

/// On-Balance Volume
pub fn obv(bars: &[Bar]) -> Vec<f64> {
    if bars.is_empty() {
        return vec![];
    }

    let mut obv_values = Vec::with_capacity(bars.len());
    obv_values.push(bars[0].volume);

    for i in 1..bars.len() {
        let prev_obv = obv_values[i - 1];
        let new_obv = if bars[i].close > bars[i - 1].close {
            prev_obv + bars[i].volume
        } else if bars[i].close < bars[i - 1].close {
            prev_obv - bars[i].volume
        } else {
            prev_obv
        };
        obv_values.push(new_obv);
    }

    obv_values
}

/// Average Directional Index (ADX) — measures trend strength (0-100)
pub struct AdxResult {
    pub adx: Vec<f64>,
    pub plus_di: Vec<f64>,
    pub minus_di: Vec<f64>,
}

pub fn adx(bars: &[Bar], period: usize) -> AdxResult {
    if period == 0 || bars.len() < period * 2 + 1 {
        return AdxResult {
            adx: vec![],
            plus_di: vec![],
            minus_di: vec![],
        };
    }

    // Calculate +DM, -DM and TR
    let mut plus_dm = Vec::with_capacity(bars.len() - 1);
    let mut minus_dm = Vec::with_capacity(bars.len() - 1);
    let mut true_range = Vec::with_capacity(bars.len() - 1);

    for i in 1..bars.len() {
        let up_move = bars[i].high - bars[i - 1].high;
        let down_move = bars[i - 1].low - bars[i].low;

        plus_dm.push(if up_move > down_move && up_move > 0.0 {
            up_move
        } else {
            0.0
        });
        minus_dm.push(if down_move > up_move && down_move > 0.0 {
            down_move
        } else {
            0.0
        });

        let hl = bars[i].high - bars[i].low;
        let hc = (bars[i].high - bars[i - 1].close).abs();
        let lc = (bars[i].low - bars[i - 1].close).abs();
        true_range.push(hl.max(hc).max(lc));
    }

    // Smoothed averages using Wilder's method
    let mut smoothed_plus_dm = plus_dm[..period].iter().sum::<f64>();
    let mut smoothed_minus_dm = minus_dm[..period].iter().sum::<f64>();
    let mut smoothed_tr = true_range[..period].iter().sum::<f64>();

    let mut plus_di_values = Vec::new();
    let mut minus_di_values = Vec::new();
    let mut dx_values = Vec::new();

    for i in period..plus_dm.len() {
        smoothed_plus_dm = smoothed_plus_dm - smoothed_plus_dm / period as f64 + plus_dm[i];
        smoothed_minus_dm = smoothed_minus_dm - smoothed_minus_dm / period as f64 + minus_dm[i];
        smoothed_tr = smoothed_tr - smoothed_tr / period as f64 + true_range[i];

        let pdi = if smoothed_tr > 0.0 {
            100.0 * smoothed_plus_dm / smoothed_tr
        } else {
            0.0
        };
        let mdi = if smoothed_tr > 0.0 {
            100.0 * smoothed_minus_dm / smoothed_tr
        } else {
            0.0
        };

        plus_di_values.push(pdi);
        minus_di_values.push(mdi);

        let di_sum = pdi + mdi;
        let dx = if di_sum > 0.0 {
            100.0 * (pdi - mdi).abs() / di_sum
        } else {
            0.0
        };
        dx_values.push(dx);
    }

    // Smooth DX into ADX
    if dx_values.len() < period {
        return AdxResult {
            adx: vec![],
            plus_di: plus_di_values,
            minus_di: minus_di_values,
        };
    }

    let mut adx_values = Vec::new();
    let mut adx_val = dx_values[..period].iter().sum::<f64>() / period as f64;
    adx_val = finite_or(adx_val, 0.0);
    adx_values.push(adx_val);

    for dx in &dx_values[period..] {
        adx_val = (adx_val * (period - 1) as f64 + dx) / period as f64;
        adx_values.push(finite_or(adx_val, 0.0));
    }

    AdxResult {
        adx: adx_values,
        plus_di: plus_di_values,
        minus_di: minus_di_values,
    }
}

/// Support and resistance levels from recent pivot points
pub struct SupportResistance {
    pub support: Option<f64>,
    pub resistance: Option<f64>,
}

pub fn support_resistance(bars: &[Bar], lookback: usize) -> SupportResistance {
    if bars.len() < lookback + 2 {
        return SupportResistance {
            support: None,
            resistance: None,
        };
    }

    let recent = &bars[bars.len() - lookback..];
    let mut swing_highs: Vec<f64> = Vec::new();
    let mut swing_lows: Vec<f64> = Vec::new();

    // Find swing highs/lows (local extremes with 2-bar confirmation)
    for i in 2..recent.len() - 2 {
        if recent[i].high > recent[i - 1].high
            && recent[i].high > recent[i - 2].high
            && recent[i].high > recent[i + 1].high
            && recent[i].high > recent[i + 2].high
        {
            swing_highs.push(recent[i].high);
        }
        if recent[i].low < recent[i - 1].low
            && recent[i].low < recent[i - 2].low
            && recent[i].low < recent[i + 1].low
            && recent[i].low < recent[i + 2].low
        {
            swing_lows.push(recent[i].low);
        }
    }

    let current_price = bars.last().unwrap().close;

    // Nearest resistance = lowest swing high above current price
    let resistance = swing_highs
        .iter()
        .filter(|&&h| h > current_price)
        .copied()
        .reduce(f64::min);

    // Nearest support = highest swing low below current price
    let support = swing_lows
        .iter()
        .filter(|&&l| l < current_price)
        .copied()
        .reduce(f64::max);

    SupportResistance {
        support,
        resistance,
    }
}

/// Volume-Weighted Average Price
pub fn vwap(bars: &[Bar]) -> Vec<f64> {
    if bars.is_empty() {
        return vec![];
    }

    let mut vwap_values = Vec::with_capacity(bars.len());
    let mut cumulative_tpv = 0.0;
    let mut cumulative_volume = 0.0;

    for bar in bars {
        let typical_price = (bar.high + bar.low + bar.close) / 3.0;
        cumulative_tpv += typical_price * bar.volume;
        cumulative_volume += bar.volume;

        let vwap = if cumulative_volume > 0.0 {
            cumulative_tpv / cumulative_volume
        } else {
            typical_price
        };

        vwap_values.push(finite_or(vwap, typical_price));
    }

    vwap_values
}

/// Ichimoku Cloud components
pub struct IchimokuResult {
    pub tenkan_sen: Vec<f64>,    // Conversion Line (9-period)
    pub kijun_sen: Vec<f64>,     // Base Line (26-period)
    pub senkou_span_a: Vec<f64>, // Leading Span A
    pub senkou_span_b: Vec<f64>, // Leading Span B
    pub chikou_span: Vec<f64>,   // Lagging Span
}

/// Calculate the high/low midpoint over a period
fn period_midpoint(bars: &[Bar], end: usize, period: usize) -> f64 {
    let start = if end >= period { end - period + 1 } else { 0 };
    let slice = &bars[start..=end];
    let high = slice
        .iter()
        .map(|b| b.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let low = slice.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    (high + low) / 2.0
}

/// Ichimoku Cloud — tenkan(9), kijun(26), senkou spans, chikou
pub fn ichimoku(bars: &[Bar]) -> IchimokuResult {
    let empty = IchimokuResult {
        tenkan_sen: vec![],
        kijun_sen: vec![],
        senkou_span_a: vec![],
        senkou_span_b: vec![],
        chikou_span: vec![],
    };
    if bars.len() < 52 {
        return empty;
    }

    let n = bars.len();
    let mut tenkan = Vec::with_capacity(n);
    let mut kijun = Vec::with_capacity(n);
    let mut span_a = Vec::with_capacity(n);
    let mut span_b = Vec::with_capacity(n);

    for i in 0..n {
        tenkan.push(if i >= 8 {
            period_midpoint(bars, i, 9)
        } else {
            bars[i].close
        });
        kijun.push(if i >= 25 {
            period_midpoint(bars, i, 26)
        } else {
            bars[i].close
        });
    }

    // Senkou Span A = midpoint of tenkan & kijun (plotted 26 periods ahead)
    // Senkou Span B = 52-period midpoint (plotted 26 periods ahead)
    // We store them aligned to the current bar (i.e., the value that WAS plotted 26 bars ago)
    for i in 25..n {
        let src = i - 25; // value from 26 bars ago, shifted forward to current
        span_a.push((tenkan[src] + kijun[src]) / 2.0);
        span_b.push(if src >= 51 {
            period_midpoint(bars, src, 52)
        } else {
            bars[src].close
        });
    }

    // Chikou span = close shifted 26 periods back (so for display, chikou[i] is close[i+26])
    let chikou: Vec<f64> = if n > 26 {
        bars[26..].iter().map(|b| b.close).collect()
    } else {
        vec![]
    };

    IchimokuResult {
        tenkan_sen: tenkan,
        kijun_sen: kijun,
        senkou_span_a: span_a,
        senkou_span_b: span_b,
        chikou_span: chikou,
    }
}

/// Fibonacci retracement levels between a swing low and swing high
pub struct FibonacciLevels {
    pub level_236: f64,
    pub level_382: f64,
    pub level_500: f64,
    pub level_618: f64,
    pub level_786: f64,
    pub swing_high: f64,
    pub swing_low: f64,
}

/// Compute Fibonacci retracement levels from recent bars
pub fn fibonacci_retracement(bars: &[Bar], lookback: usize) -> Option<FibonacciLevels> {
    if bars.len() < lookback {
        return None;
    }
    let recent = &bars[bars.len() - lookback..];
    let swing_high = recent
        .iter()
        .map(|b| b.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let swing_low = recent.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let diff = swing_high - swing_low;
    if diff <= 0.0 {
        return None;
    }

    Some(FibonacciLevels {
        level_236: swing_high - diff * 0.236,
        level_382: swing_high - diff * 0.382,
        level_500: swing_high - diff * 0.500,
        level_618: swing_high - diff * 0.618,
        level_786: swing_high - diff * 0.786,
        swing_high,
        swing_low,
    })
}

/// Resample daily bars into weekly OHLCV bars
pub fn resample_to_weekly(bars: &[Bar]) -> Vec<Bar> {
    use chrono::Datelike;
    if bars.is_empty() {
        return vec![];
    }

    let mut weekly: Vec<Bar> = Vec::new();
    let mut week_open = bars[0].open;
    let mut week_high = bars[0].high;
    let mut week_low = bars[0].low;
    let mut week_close = bars[0].close;
    let mut week_volume = bars[0].volume;
    let mut week_start = bars[0].timestamp;
    let mut current_iso = (
        bars[0].timestamp.iso_week().year(),
        bars[0].timestamp.iso_week().week(),
    );

    for bar in bars.iter().skip(1) {
        let iso = (
            bar.timestamp.iso_week().year(),
            bar.timestamp.iso_week().week(),
        );
        if iso != current_iso {
            weekly.push(Bar {
                timestamp: week_start,
                open: week_open,
                high: week_high,
                low: week_low,
                close: week_close,
                volume: week_volume,
                vwap: None,
            });
            week_open = bar.open;
            week_high = bar.high;
            week_low = bar.low;
            week_volume = 0.0;
            week_start = bar.timestamp;
            current_iso = iso;
        }
        week_high = week_high.max(bar.high);
        week_low = week_low.min(bar.low);
        week_close = bar.close;
        week_volume += bar.volume;
    }

    weekly.push(Bar {
        timestamp: week_start,
        open: week_open,
        high: week_high,
        low: week_low,
        close: week_close,
        volume: week_volume,
        vwap: None,
    });

    weekly
}

/// Relative strength line: stock / benchmark price ratio
pub fn relative_strength(stock_closes: &[f64], benchmark_closes: &[f64]) -> Vec<f64> {
    let n = stock_closes.len().min(benchmark_closes.len());
    let s_offset = stock_closes.len() - n;
    let b_offset = benchmark_closes.len() - n;
    (0..n)
        .map(|i| {
            let b = benchmark_closes[b_offset + i];
            if b > 0.0 {
                stock_closes[s_offset + i] / b
            } else {
                1.0
            }
        })
        .collect()
}

/// Keltner Channels (EMA +/- ATR × multiplier)
pub struct KeltnerChannels {
    pub upper: Vec<f64>,
    pub middle: Vec<f64>,
    pub lower: Vec<f64>,
}

pub fn keltner_channels(
    bars: &[Bar],
    ema_period: usize,
    atr_period: usize,
    multiplier: f64,
) -> KeltnerChannels {
    if bars.len() < ema_period.max(atr_period + 1) {
        return KeltnerChannels {
            upper: vec![],
            middle: vec![],
            lower: vec![],
        };
    }

    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let middle = ema(&closes, ema_period);
    let atr_values = atr(bars, atr_period);

    // Align: EMA starts at index ema_period-1, ATR starts at index atr_period
    let offset = (atr_period + 1).saturating_sub(ema_period);
    let mut upper = Vec::new();
    let mut lower = Vec::new();

    for (atr_idx, mid_val) in middle[offset..].iter().enumerate() {
        if atr_idx < atr_values.len() {
            upper.push(mid_val + multiplier * atr_values[atr_idx]);
            lower.push(mid_val - multiplier * atr_values[atr_idx]);
        }
    }

    // Trim middle to match
    let trimmed_middle = middle[offset..offset + upper.len()].to_vec();

    KeltnerChannels {
        upper,
        middle: trimmed_middle,
        lower,
    }
}

/// Pivot Points (Classic Floor Trader's Pivots)
pub struct PivotPoints {
    pub pivot: f64,
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
    pub s1: f64,
    pub s2: f64,
    pub s3: f64,
}

pub fn pivot_points(bars: &[Bar]) -> Option<PivotPoints> {
    let bar = bars.last()?;
    let pivot = (bar.high + bar.low + bar.close) / 3.0;

    let r1 = 2.0 * pivot - bar.low;
    let s1 = 2.0 * pivot - bar.high;
    let r2 = pivot + (bar.high - bar.low);
    let s2 = pivot - (bar.high - bar.low);
    let r3 = bar.high + 2.0 * (pivot - bar.low);
    let s3 = bar.low - 2.0 * (bar.high - pivot);

    Some(PivotPoints {
        pivot,
        r1,
        r2,
        r3,
        s1,
        s2,
        s3,
    })
}

/// Market Structure: detect higher highs/higher lows (uptrend) or lower lows/lower highs (downtrend)
pub struct MarketStructure {
    pub higher_highs: usize,
    pub lower_lows: usize,
    pub higher_lows: usize,
    pub lower_highs: usize,
}

pub fn market_structure(bars: &[Bar], lookback: usize) -> MarketStructure {
    if bars.len() < lookback + 1 {
        return MarketStructure {
            higher_highs: 0,
            lower_lows: 0,
            higher_lows: 0,
            lower_highs: 0,
        };
    }

    let recent = &bars[bars.len() - lookback..];
    let mut highs = Vec::new();
    let mut lows = Vec::new();

    // Find local swing highs and lows
    for i in 1..recent.len() - 1 {
        if recent[i].high > recent[i - 1].high && recent[i].high > recent[i + 1].high {
            highs.push((i, recent[i].high));
        }
        if recent[i].low < recent[i - 1].low && recent[i].low < recent[i + 1].low {
            lows.push((i, recent[i].low));
        }
    }

    let mut hh = 0;
    let mut lh = 0;
    for i in 1..highs.len() {
        if highs[i].1 > highs[i - 1].1 {
            hh += 1;
        } else {
            lh += 1;
        }
    }

    let mut hl = 0;
    let mut ll = 0;
    for i in 1..lows.len() {
        if lows[i].1 > lows[i - 1].1 {
            hl += 1;
        } else {
            ll += 1;
        }
    }

    MarketStructure {
        higher_highs: hh,
        lower_lows: ll,
        higher_lows: hl,
        lower_highs: lh,
    }
}
