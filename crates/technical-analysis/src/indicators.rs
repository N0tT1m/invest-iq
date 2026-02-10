use analysis_core::Bar;

/// Simple Moving Average
pub fn sma(data: &[f64], period: usize) -> Vec<f64> {
    if period == 0 || data.len() < period {
        return vec![];
    }

    let mut result = Vec::with_capacity(data.len() - period + 1);
    for i in period - 1..data.len() {
        let sum: f64 = data[i + 1 - period..=i].iter().sum();
        result.push(sum / period as f64);
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

    // Start with SMA for first value
    if data.len() < period {
        return vec![data.iter().sum::<f64>() / data.len() as f64];
    }

    let sma: f64 = data[..period].iter().sum::<f64>() / period as f64;
    result.push(sma);

    for i in 1..data.len() {
        let ema_val = (data[i] - result[i - 1]) * multiplier + result[i - 1];
        result.push(ema_val);
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
        rsi_values.push(rsi);
    }

    rsi_values
}

/// MACD (Moving Average Convergence Divergence)
pub struct MacdResult {
    pub macd_line: Vec<f64>,
    pub signal_line: Vec<f64>,
    pub histogram: Vec<f64>,
}

pub fn macd(data: &[f64], fast_period: usize, slow_period: usize, signal_period: usize) -> MacdResult {
    if fast_period == 0 || slow_period == 0 || signal_period == 0 || slow_period < fast_period {
        return MacdResult { macd_line: vec![], signal_line: vec![], histogram: vec![] };
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
        return BollingerBands { upper: vec![], middle: vec![], lower: vec![] };
    }

    let middle = sma(data, period);
    let mut upper = Vec::with_capacity(middle.len());
    let mut lower = Vec::with_capacity(middle.len());

    for i in period - 1..data.len() {
        let slice = &data[i + 1 - period..=i];
        let mean = middle[i + 1 - period];
        let variance: f64 = slice.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / period as f64;
        let std = variance.sqrt();

        upper.push(mean + std_dev * std);
        lower.push(mean - std_dev * std);
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
    atr_values.push(atr);

    for i in period..true_ranges.len() {
        atr = (atr * (period - 1) as f64 + true_ranges[i]) / period as f64;
        atr_values.push(atr);
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
        return StochasticResult { k: vec![], d: vec![] };
    }

    let mut k_values = Vec::new();

    for i in k_period - 1..bars.len() {
        let slice = &bars[i + 1 - k_period..=i];
        let highest = slice.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let lowest = slice.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);

        let k = if highest == lowest {
            50.0
        } else {
            100.0 * (bars[i].close - lowest) / (highest - lowest)
        };

        k_values.push(k);
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
        return AdxResult { adx: vec![], plus_di: vec![], minus_di: vec![] };
    }

    // Calculate +DM, -DM and TR
    let mut plus_dm = Vec::with_capacity(bars.len() - 1);
    let mut minus_dm = Vec::with_capacity(bars.len() - 1);
    let mut true_range = Vec::with_capacity(bars.len() - 1);

    for i in 1..bars.len() {
        let up_move = bars[i].high - bars[i - 1].high;
        let down_move = bars[i - 1].low - bars[i].low;

        plus_dm.push(if up_move > down_move && up_move > 0.0 { up_move } else { 0.0 });
        minus_dm.push(if down_move > up_move && down_move > 0.0 { down_move } else { 0.0 });

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

        let pdi = if smoothed_tr > 0.0 { 100.0 * smoothed_plus_dm / smoothed_tr } else { 0.0 };
        let mdi = if smoothed_tr > 0.0 { 100.0 * smoothed_minus_dm / smoothed_tr } else { 0.0 };

        plus_di_values.push(pdi);
        minus_di_values.push(mdi);

        let di_sum = pdi + mdi;
        let dx = if di_sum > 0.0 { 100.0 * (pdi - mdi).abs() / di_sum } else { 0.0 };
        dx_values.push(dx);
    }

    // Smooth DX into ADX
    if dx_values.len() < period {
        return AdxResult { adx: vec![], plus_di: plus_di_values, minus_di: minus_di_values };
    }

    let mut adx_values = Vec::new();
    let mut adx_val = dx_values[..period].iter().sum::<f64>() / period as f64;
    adx_values.push(adx_val);

    for i in period..dx_values.len() {
        adx_val = (adx_val * (period - 1) as f64 + dx_values[i]) / period as f64;
        adx_values.push(adx_val);
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
        return SupportResistance { support: None, resistance: None };
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

    SupportResistance { support, resistance }
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

        vwap_values.push(vwap);
    }

    vwap_values
}
