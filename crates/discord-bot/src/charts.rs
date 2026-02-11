use analysis_core::{Bar, SignalStrength};
use anyhow::Result;
use plotters::prelude::*;
use std::path::PathBuf;

const CHART_WIDTH: u32 = 1400;
const CHART_HEIGHT: u32 = 1000;

// Layout percentages (of total height)
const PRICE_RATIO: f64 = 0.50;
const VOLUME_RATIO: f64 = 0.10;
const RSI_RATIO: f64 = 0.20;
// MACD gets the remaining 0.20

pub async fn generate_chart(
    symbol: &str,
    bars: &[Bar],
    signal: &SignalStrength,
) -> Result<PathBuf> {
    if bars.is_empty() {
        return Err(anyhow::anyhow!("No data to chart"));
    }

    let filename = format!(
        "/tmp/investiq_{}_{}.png",
        symbol,
        chrono::Utc::now().timestamp()
    );
    let path = PathBuf::from(&filename);

    let root =
        BitMapBackend::new(&filename, (CHART_WIDTH, CHART_HEIGHT)).into_drawing_area();
    root.fill(&WHITE)?;

    let price_h = (CHART_HEIGHT as f64 * PRICE_RATIO) as u32;
    let volume_h = (CHART_HEIGHT as f64 * VOLUME_RATIO) as u32;
    let rsi_h = (CHART_HEIGHT as f64 * RSI_RATIO) as u32;

    let (price_area, rest) = root.split_vertically(price_h);
    let (volume_area, rest2) = rest.split_vertically(volume_h);
    let (rsi_area, macd_area) = rest2.split_vertically(rsi_h);

    draw_price_chart(&price_area, symbol, bars, signal)?;
    draw_volume_chart(&volume_area, bars)?;
    draw_rsi_chart(&rsi_area, bars)?;
    draw_macd_chart(&macd_area, bars)?;

    root.present()?;

    Ok(path)
}

fn draw_price_chart(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    symbol: &str,
    bars: &[Bar],
    signal: &SignalStrength,
) -> Result<()> {
    let min_price = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let max_price = bars
        .iter()
        .map(|b| b.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let price_range = max_price - min_price;
    let y_min = min_price - price_range * 0.1;
    let y_max = max_price + price_range * 0.1;

    let x_range = 0..bars.len();

    let signal_color = match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy => GREEN,
        SignalStrength::WeakBuy => CYAN,
        SignalStrength::Neutral => YELLOW,
        SignalStrength::WeakSell => RGBColor(255, 140, 0),
        SignalStrength::Sell | SignalStrength::StrongSell => RED,
    };

    let mut chart = ChartBuilder::on(area)
        .caption(
            format!("{} - {:?}", symbol, signal),
            ("sans-serif", 30, &signal_color),
        )
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(70)
        .build_cartesian_2d(x_range.clone(), y_min..y_max)?;

    chart
        .configure_mesh()
        .x_labels(10)
        .y_labels(10)
        .x_label_formatter(&|x| {
            if *x < bars.len() {
                bars[*x].timestamp.format("%m/%d").to_string()
            } else {
                String::new()
            }
        })
        .y_label_formatter(&|y| format!("${:.2}", y))
        .draw()?;

    // Draw Bollinger Bands (before candles so they appear behind)
    if bars.len() >= 20 {
        let (upper, middle, lower) = calculate_bollinger_bands(bars, 20, 2.0);
        let offset = 19;

        // Shaded band area
        let band_points: Vec<(usize, f64, f64)> = (0..upper.len())
            .map(|i| (i + offset, upper[i], lower[i]))
            .collect();

        for w in band_points.windows(2) {
            let (x0, u0, l0) = w[0];
            let (x1, u1, l1) = w[1];
            // Draw filled polygon for band area
            let _ = chart.draw_series(std::iter::once(Polygon::new(
                vec![(x0, u0), (x1, u1), (x1, l1), (x0, l0)],
                RGBColor(100, 149, 237).mix(0.15).filled(),
            )));
        }

        // Upper band line
        let upper_data: Vec<(usize, f64)> =
            upper.iter().enumerate().map(|(i, &v)| (i + offset, v)).collect();
        chart
            .draw_series(LineSeries::new(upper_data, &RGBColor(100, 149, 237).mix(0.5)))?
            .label("BB Upper")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], RGBColor(100, 149, 237))
            });

        // Lower band line
        let lower_data: Vec<(usize, f64)> =
            lower.iter().enumerate().map(|(i, &v)| (i + offset, v)).collect();
        chart.draw_series(LineSeries::new(
            lower_data,
            &RGBColor(100, 149, 237).mix(0.5),
        ))?;

        // Middle band (SMA 20) - drawn as part of BB
        let mid_data: Vec<(usize, f64)> =
            middle.iter().enumerate().map(|(i, &v)| (i + offset, v)).collect();
        chart
            .draw_series(LineSeries::new(mid_data, &BLUE.mix(0.8)))?
            .label("SMA 20")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));
    }

    // Draw candlesticks
    chart.draw_series(bars.iter().enumerate().map(|(idx, bar)| {
        let color = if bar.close >= bar.open { &GREEN } else { &RED };
        CandleStick::new(
            idx,
            bar.open,
            bar.high,
            bar.low,
            bar.close,
            color.filled(),
            color,
            3,
        )
    }))?;

    // Draw SMA 50
    if bars.len() >= 50 {
        let sma50: Vec<(usize, f64)> = bars
            .windows(50)
            .enumerate()
            .map(|(i, window)| {
                let avg = window.iter().map(|b| b.close).sum::<f64>() / 50.0;
                (i + 49, avg)
            })
            .collect();

        chart
            .draw_series(LineSeries::new(sma50, &RGBColor(255, 0, 255).mix(0.7)))?
            .label("SMA 50")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], RGBColor(255, 0, 255))
            });
    }

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .position(SeriesLabelPosition::UpperLeft)
        .draw()?;

    Ok(())
}

fn draw_volume_chart(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    bars: &[Bar],
) -> Result<()> {
    if bars.is_empty() {
        return Ok(());
    }

    let max_vol = bars
        .iter()
        .map(|b| b.volume)
        .fold(0.0f64, f64::max)
        * 1.1;

    if max_vol <= 0.0 {
        return Ok(());
    }

    let mut chart = ChartBuilder::on(area)
        .caption("Volume", ("sans-serif", 16))
        .margin(10)
        .x_label_area_size(20)
        .y_label_area_size(70)
        .build_cartesian_2d(0..bars.len(), 0.0..max_vol)?;

    chart
        .configure_mesh()
        .x_labels(0)
        .y_labels(3)
        .y_label_formatter(&|y| format_volume(*y))
        .draw()?;

    // Volume bars colored by price direction
    for (idx, bar) in bars.iter().enumerate() {
        let color = if bar.close >= bar.open {
            GREEN.mix(0.6)
        } else {
            RED.mix(0.6)
        };
        chart.draw_series(std::iter::once(Rectangle::new(
            [(idx, 0.0), (idx, bar.volume)],
            color.filled(),
        )))?;
    }

    // 20-day volume SMA
    if bars.len() >= 20 {
        let vol_sma: Vec<(usize, f64)> = bars
            .windows(20)
            .enumerate()
            .map(|(i, window)| {
                let avg = window.iter().map(|b| b.volume).sum::<f64>() / 20.0;
                (i + 19, avg)
            })
            .collect();

        chart.draw_series(LineSeries::new(vol_sma, &BLUE.mix(0.7)))?;
    }

    Ok(())
}

fn draw_rsi_chart(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    bars: &[Bar],
) -> Result<()> {
    if bars.len() < 15 {
        return Ok(());
    }

    let rsi = calculate_rsi(bars, 14);

    let mut chart = ChartBuilder::on(area)
        .caption("RSI (14)", ("sans-serif", 16))
        .margin(10)
        .x_label_area_size(20)
        .y_label_area_size(70)
        .build_cartesian_2d(0..bars.len(), 0.0..100.0)?;

    chart.configure_mesh().x_labels(0).y_labels(5).draw()?;

    // Overbought/Oversold zones
    chart.draw_series(LineSeries::new(
        vec![(0, 70.0), (bars.len(), 70.0)],
        &RED.mix(0.3),
    ))?;
    chart.draw_series(LineSeries::new(
        vec![(0, 30.0), (bars.len(), 30.0)],
        &GREEN.mix(0.3),
    ))?;
    chart.draw_series(LineSeries::new(
        vec![(0, 50.0), (bars.len(), 50.0)],
        &BLACK.mix(0.2),
    ))?;

    // RSI line
    let rsi_data: Vec<(usize, f64)> = rsi
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i + 14, v))
        .collect();
    chart.draw_series(LineSeries::new(rsi_data, &CYAN.mix(0.8)))?;

    Ok(())
}

fn draw_macd_chart(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    bars: &[Bar],
) -> Result<()> {
    if bars.len() < 26 {
        return Ok(());
    }

    let (macd, signal, histogram) = calculate_macd(bars, 12, 26, 9);

    let max_val = histogram
        .iter()
        .map(|v| v.abs())
        .fold(0.0f64, f64::max)
        * 1.2;

    if max_val == 0.0 {
        return Ok(());
    }

    let mut chart = ChartBuilder::on(area)
        .caption("MACD", ("sans-serif", 16))
        .margin(10)
        .x_label_area_size(20)
        .y_label_area_size(70)
        .build_cartesian_2d(0..bars.len(), -max_val..max_val)?;

    chart.configure_mesh().x_labels(0).y_labels(5).draw()?;

    // Zero line
    chart.draw_series(LineSeries::new(
        vec![(0, 0.0), (bars.len(), 0.0)],
        &BLACK.mix(0.3),
    ))?;

    // Histogram
    let hist_data: Vec<(usize, f64)> = histogram
        .iter()
        .enumerate()
        .map(|(i, &v)| (i + 33, v))
        .collect();
    for (idx, val) in hist_data.iter() {
        let color = if *val >= 0.0 { &GREEN } else { &RED };
        chart.draw_series(std::iter::once(Rectangle::new(
            [(*idx, 0.0), (*idx, *val)],
            color.mix(0.6).filled(),
        )))?;
    }

    // MACD line
    let macd_data: Vec<(usize, f64)> = macd
        .iter()
        .enumerate()
        .map(|(i, &v)| (i + 25, v))
        .collect();
    chart.draw_series(LineSeries::new(macd_data, &BLUE.mix(0.8)))?;

    // Signal line
    let signal_data: Vec<(usize, f64)> = signal
        .iter()
        .enumerate()
        .map(|(i, &v)| (i + 33, v))
        .collect();
    chart.draw_series(LineSeries::new(
        signal_data,
        &Palette99::pick(8).mix(0.8),
    ))?;

    Ok(())
}

fn calculate_bollinger_bands(
    bars: &[Bar],
    period: usize,
    num_std: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut upper = Vec::new();
    let mut middle = Vec::new();
    let mut lower = Vec::new();

    for window in bars.windows(period) {
        let mean = window.iter().map(|b| b.close).sum::<f64>() / period as f64;
        let variance =
            window.iter().map(|b| (b.close - mean).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        upper.push(mean + num_std * std_dev);
        middle.push(mean);
        lower.push(mean - num_std * std_dev);
    }

    (upper, middle, lower)
}

fn format_volume(v: f64) -> String {
    if v >= 1_000_000_000.0 {
        format!("{:.1}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.0}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

fn calculate_rsi(bars: &[Bar], period: usize) -> Vec<f64> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let mut gains = Vec::new();
    let mut losses = Vec::new();

    for i in 1..closes.len() {
        let change = closes[i] - closes[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(change.abs());
        }
    }

    if gains.len() < period {
        return Vec::new();
    }

    let mut avg_gain = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss = losses[..period].iter().sum::<f64>() / period as f64;

    let mut rsi_values = Vec::new();

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

fn calculate_macd(
    bars: &[Bar],
    fast: usize,
    slow: usize,
    signal: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();

    let ema_fast = calculate_ema(&closes, fast);
    let ema_slow = calculate_ema(&closes, slow);

    let mut macd = Vec::new();
    for i in 0..ema_fast.len().min(ema_slow.len()) {
        macd.push(ema_fast[i] - ema_slow[i]);
    }

    let signal_line = calculate_ema(&macd, signal);

    let mut histogram = Vec::new();
    for i in 0..macd.len().min(signal_line.len()) {
        histogram.push(macd[i] - signal_line[i]);
    }

    (macd, signal_line, histogram)
}

fn calculate_ema(data: &[f64], period: usize) -> Vec<f64> {
    if data.is_empty() || data.len() < period {
        return Vec::new();
    }

    let mut result = Vec::new();
    let multiplier = 2.0 / (period as f64 + 1.0);

    let sma: f64 = data[..period].iter().sum::<f64>() / period as f64;
    result.push(sma);

    for val in &data[period..] {
        let prev = result[result.len() - 1];
        let ema = (val - prev) * multiplier + prev;
        result.push(ema);
    }

    result
}
