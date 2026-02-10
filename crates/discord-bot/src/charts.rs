use analysis_core::{Bar, SignalStrength};
use anyhow::Result;
use plotters::prelude::*;
use std::path::PathBuf;

const CHART_WIDTH: u32 = 1200;
const CHART_HEIGHT: u32 = 900;

pub async fn generate_chart(
    symbol: &str,
    bars: &[Bar],
    signal: &SignalStrength,
) -> Result<PathBuf> {
    if bars.is_empty() {
        return Err(anyhow::anyhow!("No data to chart"));
    }

    let filename = format!("/tmp/investiq_{}_{}.png", symbol, chrono::Utc::now().timestamp());
    let path = PathBuf::from(&filename);

    let root = BitMapBackend::new(&filename, (CHART_WIDTH, CHART_HEIGHT))
        .into_drawing_area();
    root.fill(&WHITE)?;

    // Split into 3 sections: main chart (60%), RSI (20%), MACD (20%)
    let (main_area, lower_area) = root.split_vertically(CHART_HEIGHT * 6 / 10);
    let (rsi_area, macd_area) = lower_area.split_vertically(CHART_HEIGHT * 2 / 10);

    // Draw main price chart
    draw_price_chart(&main_area, symbol, bars, signal)?;

    // Draw RSI
    draw_rsi_chart(&rsi_area, bars)?;

    // Draw MACD
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
    let max_price = bars.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    let price_range = max_price - min_price;
    let y_min = min_price - price_range * 0.1;
    let y_max = max_price + price_range * 0.1;

    let x_range = 0..bars.len();

    let signal_color = match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy => GREEN,
        SignalStrength::WeakBuy => CYAN,
        SignalStrength::Neutral => YELLOW,
        SignalStrength::WeakSell => RGBColor(255, 140, 0), // Orange
        SignalStrength::Sell | SignalStrength::StrongSell => RED,
    };

    let mut chart = ChartBuilder::on(area)
        .caption(
            format!("{} - {:?}", symbol, signal),
            ("sans-serif", 30, &signal_color),
        )
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(60)
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

    // Draw candlesticks
    chart.draw_series(
        bars.iter().enumerate().map(|(idx, bar)| {
            let color = if bar.close >= bar.open {
                &GREEN
            } else {
                &RED
            };

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
        }),
    )?;

    // Draw SMA 20
    if bars.len() >= 20 {
        let sma: Vec<(usize, f64)> = bars
            .windows(20)
            .enumerate()
            .map(|(i, window)| {
                let avg = window.iter().map(|b| b.close).sum::<f64>() / 20.0;
                (i + 19, avg)
            })
            .collect();

        chart
            .draw_series(LineSeries::new(sma, &BLUE.mix(0.8)))?
            .label("SMA 20")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));
    }

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    Ok(())
}

fn draw_rsi_chart(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    bars: &[Bar],
) -> Result<()> {
    if bars.len() < 15 {
        return Ok(());
    }

    // Calculate RSI
    let rsi = calculate_rsi(bars, 14);

    let mut chart = ChartBuilder::on(area)
        .caption("RSI (14)", ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(60)
        .build_cartesian_2d(0..bars.len(), 0.0..100.0)?;

    chart.configure_mesh().x_labels(10).y_labels(5).draw()?;

    // Overbought/Oversold lines
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
    let rsi_data: Vec<(usize, f64)> = rsi.into_iter().enumerate().map(|(i, v)| (i + 14, v)).collect();
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

    let max_val = histogram.iter().map(|v| v.abs()).fold(0.0f64, f64::max) * 1.2;

    let mut chart = ChartBuilder::on(area)
        .caption("MACD", ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(60)
        .build_cartesian_2d(0..bars.len(), -max_val..max_val)?;

    chart.configure_mesh().x_labels(10).y_labels(5).draw()?;

    // Zero line
    chart.draw_series(LineSeries::new(
        vec![(0, 0.0), (bars.len(), 0.0)],
        &BLACK.mix(0.3),
    ))?;

    // Histogram
    let hist_data: Vec<(usize, f64)> = histogram.iter().enumerate().map(|(i, &v)| (i + 33, v)).collect();
    for (idx, val) in hist_data.iter() {
        let color = if *val >= 0.0 { &GREEN } else { &RED };
        chart.draw_series(std::iter::once(Rectangle::new(
            [(*idx, 0.0), (*idx, *val)],
            color.mix(0.6).filled(),
        )))?;
    }

    // MACD line
    let macd_data: Vec<(usize, f64)> = macd.iter().enumerate().map(|(i, &v)| (i + 25, v)).collect();
    chart.draw_series(LineSeries::new(macd_data, &BLUE.mix(0.8)))?;

    // Signal line
    let signal_data: Vec<(usize, f64)> = signal.iter().enumerate().map(|(i, &v)| (i + 33, v)).collect();
    chart.draw_series(LineSeries::new(signal_data, &Palette99::pick(8).mix(0.8)))?;

    Ok(())
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

    for i in period..data.len() {
        let ema = (data[i] - result[result.len() - 1]) * multiplier + result[result.len() - 1];
        result.push(ema);
    }

    result
}
