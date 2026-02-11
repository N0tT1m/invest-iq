mod bars;
mod client;
mod news;
mod price_changes;
mod tickers;

use client::PolygonFetcher;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

const MAX_CONCURRENT: usize = 100;
const RATE_PER_MINUTE: usize = 5500;

/// Reuse a single tokio runtime across calls to avoid startup overhead.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("Failed to create tokio runtime"))
}

/// Fetch all active US stock tickers from Polygon `/v3/reference/tickers`.
///
/// Paginates automatically. Filters out tickers containing "." or "/", or longer
/// than 5 characters (warrants, units, etc.).
#[pyfunction]
#[pyo3(signature = (api_key, market="stocks", ticker_type="CS"))]
fn fetch_active_tickers(
    py: Python<'_>,
    api_key: String,
    market: &str,
    ticker_type: &str,
) -> PyResult<Vec<String>> {
    let fetcher = PolygonFetcher::new(api_key, MAX_CONCURRENT, RATE_PER_MINUTE);
    let market = market.to_string();
    let ticker_type = ticker_type.to_string();

    py.detach(|| {
        runtime()
            .block_on(tickers::fetch_active_tickers_impl(
                &fetcher,
                &market,
                &ticker_type,
            ))
            .map_err(pyo3::exceptions::PyRuntimeError::new_err)
    })
}

/// Fetch OHLCV bars for multiple symbols concurrently.
///
/// Returns dict mapping symbol -> list of bar dicts with keys:
/// timestamp, open, high, low, close, volume, vwap
#[pyfunction]
#[pyo3(signature = (api_key, symbols, days=365, timespan="day"))]
fn fetch_bars_multi(
    py: Python<'_>,
    api_key: String,
    symbols: Vec<String>,
    days: i64,
    timespan: &str,
) -> PyResult<Py<PyAny>> {
    let fetcher = Arc::new(PolygonFetcher::new(api_key, MAX_CONCURRENT, RATE_PER_MINUTE));
    let timespan = timespan.to_string();

    let raw = py.detach(|| {
        runtime().block_on(bars::fetch_bars_multi_impl(fetcher, symbols, days, &timespan))
    });

    // Convert HashMap<String, Vec<Value>> -> Python dict
    let dict = PyDict::new(py);
    for (symbol, bars) in raw {
        let py_bars: Vec<Py<PyAny>> = bars
            .into_iter()
            .filter_map(|v| json_value_to_py(py, &v).ok())
            .collect();
        if !py_bars.is_empty() {
            dict.set_item(&symbol, py_bars)?;
        }
    }
    Ok(dict.into_any().unbind())
}

/// Fetch news articles for multiple symbols concurrently.
///
/// Returns dict mapping symbol -> list of article dicts with keys:
/// title, description, published_utc, tickers
#[pyfunction]
#[pyo3(signature = (api_key, symbols, limit_per_symbol=50))]
fn fetch_news_multi(
    py: Python<'_>,
    api_key: String,
    symbols: Vec<String>,
    limit_per_symbol: usize,
) -> PyResult<Py<PyAny>> {
    let fetcher = Arc::new(PolygonFetcher::new(api_key, MAX_CONCURRENT, RATE_PER_MINUTE));

    let raw = py.detach(|| {
        runtime().block_on(news::fetch_news_multi_impl(
            fetcher,
            symbols,
            limit_per_symbol,
        ))
    });

    let dict = PyDict::new(py);
    for (symbol, articles) in raw {
        let py_articles: Vec<Py<PyAny>> = articles
            .into_iter()
            .filter_map(|v| json_value_to_py(py, &v).ok())
            .collect();
        if !py_articles.is_empty() {
            dict.set_item(&symbol, py_articles)?;
        }
    }
    Ok(dict.into_any().unbind())
}

/// Fetch recent bars and compute N-day price returns for multiple symbols concurrently.
///
/// Returns dict mapping symbol -> percent change (float).
#[pyfunction]
#[pyo3(signature = (api_key, symbols, days=5))]
fn fetch_price_changes(
    py: Python<'_>,
    api_key: String,
    symbols: Vec<String>,
    days: i64,
) -> PyResult<HashMap<String, f64>> {
    let fetcher = Arc::new(PolygonFetcher::new(api_key, MAX_CONCURRENT, RATE_PER_MINUTE));

    let result = py.detach(|| {
        runtime().block_on(price_changes::fetch_price_changes_impl(
            fetcher, symbols, days,
        ))
    });

    Ok(result)
}

/// Convert a serde_json::Value to a Python object.
fn json_value_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        serde_json::Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        serde_json::Value::Array(arr) => {
            let items: Vec<Py<PyAny>> = arr
                .iter()
                .map(|v| json_value_to_py(py, v))
                .collect::<PyResult<_>>()?;
            Ok(items.into_pyobject(py)?.into_any().unbind())
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, json_value_to_py(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

/// Python module definition.
#[pymodule]
fn invest_iq_data(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fetch_active_tickers, m)?)?;
    m.add_function(wrap_pyfunction!(fetch_bars_multi, m)?)?;
    m.add_function(wrap_pyfunction!(fetch_news_multi, m)?)?;
    m.add_function(wrap_pyfunction!(fetch_price_changes, m)?)?;
    Ok(())
}
