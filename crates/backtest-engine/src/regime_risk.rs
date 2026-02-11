use crate::models::RegimeConfig;

/// Detected market regime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Regime {
    HighVol,
    LowVol,
    Normal,
}

/// Detect the current market regime from recent bar returns.
pub fn detect_regime(daily_returns: &[f64], config: &RegimeConfig) -> Regime {
    if daily_returns.len() < 5 {
        return Regime::Normal;
    }

    // Use last `lookback_bars` returns (or all if fewer)
    let lookback = config.lookback_bars.min(daily_returns.len());
    let recent = &daily_returns[daily_returns.len() - lookback..];

    let n = recent.len() as f64;
    let mean = recent.iter().sum::<f64>() / n;
    let variance = recent.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let daily_vol = variance.sqrt();

    // Annualize (sqrt(252))
    let annualized_vol = daily_vol * 252.0_f64.sqrt();

    if annualized_vol > config.high_vol_threshold {
        Regime::HighVol
    } else if annualized_vol < config.low_vol_threshold {
        Regime::LowVol
    } else {
        Regime::Normal
    }
}

/// Get the position size multiplier for the current regime.
pub fn regime_size_multiplier(regime: Regime, config: &RegimeConfig) -> f64 {
    match regime {
        Regime::HighVol => config.high_vol_multiplier,
        Regime::LowVol => config.low_vol_multiplier,
        Regime::Normal => 1.0,
    }
}
