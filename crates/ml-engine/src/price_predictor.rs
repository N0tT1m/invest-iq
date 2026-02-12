use ml_client::error::{MLError, MLResult};
use ml_client::price_predictor::{DirectionPrediction, PriceData};
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::collections::HashMap;

/// In-process wrapper around `price_predictor.model.PricePredictorInference`.
pub struct EmbeddedPricePredictor {
    model: Py<PyAny>,
}

unsafe impl Send for EmbeddedPricePredictor {}
unsafe impl Sync for EmbeddedPricePredictor {}

impl EmbeddedPricePredictor {
    pub fn load(model_path: &str, device: &str) -> MLResult<Self> {
        Python::attach(|py| {
            let module = py.import("price_predictor.model").map_err(|e: PyErr| {
                MLError::Other(format!("Failed to import price_predictor.model: {e}"))
            })?;

            let model = module
                .getattr("PricePredictorInference")
                .map_err(|e: PyErr| {
                    MLError::Other(format!("PricePredictorInference not found: {e}"))
                })?
                .call1((
                    model_path, device, false, // compile -- skip for startup speed
                ))
                .map_err(|e: PyErr| {
                    MLError::Other(format!("PricePredictorInference init failed: {e}"))
                })?;

            Ok(Self {
                model: model.unbind(),
            })
        })
    }

    pub fn predict_sync(
        &self,
        _symbol: &str,
        history: &[PriceData],
        horizon_steps: i32,
    ) -> MLResult<DirectionPrediction> {
        Python::attach(|py| {
            let np = py
                .import("numpy")
                .map_err(|e: PyErr| MLError::Other(format!("numpy not found: {e}")))?;

            // Build a 2D array: (context_length, num_features)
            // Features: open, high, low, close, volume, vwap
            let rows: Vec<Vec<f64>> = history
                .iter()
                .map(|p| {
                    vec![
                        p.open,
                        p.high,
                        p.low,
                        p.close,
                        p.volume,
                        p.vwap.unwrap_or(p.close),
                    ]
                })
                .collect();

            let py_rows = PyList::new(
                py,
                rows.iter()
                    .map(|row| PyList::new(py, row).unwrap().into_any()),
            )
            .map_err(|e: PyErr| MLError::Other(format!("Failed to create nested list: {e}")))?;

            let arr = np
                .call_method1("array", (py_rows,))
                .map_err(|e: PyErr| MLError::Other(format!("numpy.array() failed: {e}")))?;

            let result = self
                .model
                .call_method1(py, "predict_next_direction", (arr, horizon_steps))
                .map_err(|e: PyErr| {
                    MLError::Other(format!("predict_next_direction() failed: {e}"))
                })?;

            let dict = result.bind(py);

            let direction: String = dict
                .get_item("direction")
                .map_err(|e: PyErr| MLError::Other(format!("missing direction: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            let confidence: f64 = dict
                .get_item("confidence")
                .map_err(|e: PyErr| MLError::Other(format!("missing confidence: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            let probabilities: HashMap<String, f64> = dict
                .get_item("probabilities")
                .map_err(|e: PyErr| MLError::Other(format!("missing probabilities: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract probabilities: {e}"))
                })?;

            let predicted_prices: Vec<f64> = dict
                .get_item("predicted_prices")
                .map_err(|e: PyErr| MLError::Other(format!("missing predicted_prices: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract predicted_prices: {e}"))
                })?;

            let h_steps: i32 = dict
                .get_item("horizon_steps")
                .map_err(|e: PyErr| MLError::Other(format!("missing horizon_steps: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            Ok(DirectionPrediction {
                direction,
                confidence,
                probabilities,
                horizon_steps: h_steps,
                predicted_prices,
            })
        })
    }
}
