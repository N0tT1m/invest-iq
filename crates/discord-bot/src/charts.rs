use analysis_core::{Bar, SignalStrength};
use anyhow::Result;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::ffi::CStr;
use std::path::PathBuf;

pub async fn generate_chart(
    symbol: &str,
    bars: &[Bar],
    signal: &SignalStrength,
) -> Result<PathBuf> {
    if bars.is_empty() {
        return Err(anyhow::anyhow!("No data to chart"));
    }

    let symbol = symbol.to_string();
    let bars = bars.to_vec();
    let signal = *signal;

    let result = tokio::task::spawn_blocking(move || {
        generate_chart_py(&symbol, &bars, &signal)
    })
    .await??;

    Ok(result)
}

fn signal_label(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy => "Strong Buy",
        SignalStrength::Buy => "Buy",
        SignalStrength::WeakBuy => "Weak Buy",
        SignalStrength::Neutral => "Neutral",
        SignalStrength::WeakSell => "Weak Sell",
        SignalStrength::Sell => "Sell",
        SignalStrength::StrongSell => "Strong Sell",
    }
}

fn signal_color(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy => "#00C853",
        SignalStrength::WeakBuy => "#26A69A",
        SignalStrength::Neutral => "#FFD600",
        SignalStrength::WeakSell => "#FF9100",
        SignalStrength::Sell | SignalStrength::StrongSell => "#FF1744",
    }
}

fn generate_chart_py(symbol: &str, bars: &[Bar], signal: &SignalStrength) -> Result<PathBuf> {
    let filename = format!(
        "/tmp/investiq_{}_{}.png",
        symbol,
        chrono::Utc::now().timestamp()
    );

    let dates: Vec<String> = bars.iter().map(|b| b.timestamp.format("%Y-%m-%d").to_string()).collect();
    let opens: Vec<f64> = bars.iter().map(|b| b.open).collect();
    let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();

    let label = signal_label(signal);
    let color = signal_color(signal);
    let title = format!("{} - {}", symbol, label);

    Python::attach(|py| -> Result<()> {
        let code = CStr::from_bytes_with_nul(
            concat!(include_str!("chart_render.py"), "\0").as_bytes()
        ).expect("chart_render.py contains null byte");

        let module = PyModule::from_code(
            py,
            code,
            c"chart_render.py",
            c"chart_render",
        )
        .map_err(|e| anyhow::anyhow!("Failed to load chart_render.py: {}", e))?;

        module
            .getattr("render_chart")
            .map_err(|e| anyhow::anyhow!("Missing render_chart function: {}", e))?
            .call1((
                &filename,
                &title,
                color,
                dates,
                opens,
                highs,
                lows,
                closes,
                volumes,
            ))
            .map_err(|e| anyhow::anyhow!("Chart render failed: {}", e))?;

        Ok(())
    })?;

    Ok(PathBuf::from(filename))
}
