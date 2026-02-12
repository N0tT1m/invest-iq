use ml_client::bayesian::{RecommendationResponse, StrategyStats};
use ml_client::error::{MLError, MLResult};
use pyo3::prelude::*;
use std::collections::HashMap;

/// In-process wrapper around `bayesian.model.BayesianStrategyWeights`.
pub struct EmbeddedBayesian {
    model: Py<PyAny>,
}

unsafe impl Send for EmbeddedBayesian {}
unsafe impl Sync for EmbeddedBayesian {}

impl EmbeddedBayesian {
    pub fn load() -> MLResult<Self> {
        Python::attach(|py| {
            let module = py.import("bayesian.model").map_err(|e: PyErr| {
                MLError::Other(format!("Failed to import bayesian.model: {e}"))
            })?;

            let model = module
                .getattr("BayesianStrategyWeights")
                .map_err(|e: PyErr| {
                    MLError::Other(format!("BayesianStrategyWeights not found: {e}"))
                })?
                .call0()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("BayesianStrategyWeights init failed: {e}"))
                })?;

            Ok(Self {
                model: model.unbind(),
            })
        })
    }

    pub fn update_strategy_sync(&self, name: &str, outcome: i32, pnl: Option<f64>) -> MLResult<()> {
        Python::attach(|py| {
            let py_pnl = match pnl {
                Some(v) => v.into_pyobject(py).unwrap().into_any().unbind(),
                None => py.None(),
            };

            self.model
                .call_method1(py, "update_strategy", (name, outcome, py_pnl))
                .map_err(|e: PyErr| MLError::Other(format!("update_strategy() failed: {e}")))?;

            Ok(())
        })
    }

    pub fn get_weights_sync(&self, normalize: bool) -> MLResult<HashMap<String, f64>> {
        Python::attach(|py| {
            let result = self
                .model
                .call_method1(py, "get_weights", (normalize,))
                .map_err(|e: PyErr| MLError::Other(format!("get_weights() failed: {e}")))?;

            let weights: HashMap<String, f64> = result
                .bind(py)
                .extract()
                .map_err(|e: PyErr| MLError::Other(format!("Failed to extract weights: {e}")))?;

            Ok(weights)
        })
    }

    pub fn get_all_stats_sync(&self) -> MLResult<Vec<StrategyStats>> {
        Python::attach(|py| {
            let result = self
                .model
                .call_method0(py, "get_strategy_stats")
                .map_err(|e: PyErr| MLError::Other(format!("get_strategy_stats() failed: {e}")))?;

            let dict = result.bind(py);

            // Python returns {name: {alpha, beta, win_rate, total_samples, ...}, ...}
            let py_dict: HashMap<String, Py<PyAny>> = dict
                .extract()
                .map_err(|e: PyErr| MLError::Other(format!("Failed to extract stats dict: {e}")))?;

            let ci_result = self
                .model
                .call_method0(py, "get_credible_intervals")
                .map_err(|e: PyErr| {
                    MLError::Other(format!("get_credible_intervals() failed: {e}"))
                })?;
            let ci_dict: HashMap<String, (f64, f64)> =
                ci_result.bind(py).extract().unwrap_or_default();

            let weights = self.get_weights_sync(true).unwrap_or_default();

            let mut stats = Vec::new();
            for (name, obj) in &py_dict {
                let d = obj.bind(py);
                let alpha: f64 = d
                    .get_item("alpha")
                    .ok()
                    .and_then(|v| v.extract::<f64>().ok())
                    .unwrap_or(1.0);
                let beta: f64 = d
                    .get_item("beta")
                    .ok()
                    .and_then(|v| v.extract::<f64>().ok())
                    .unwrap_or(1.0);
                let win_rate: f64 = d
                    .get_item("win_rate")
                    .ok()
                    .and_then(|v| v.extract::<f64>().ok())
                    .unwrap_or(0.5);
                let total_samples: i32 = d
                    .get_item("total_samples")
                    .ok()
                    .and_then(|v| v.extract::<i32>().ok())
                    .unwrap_or(0);
                let weight = *weights.get(name).unwrap_or(&0.0);
                let ci = ci_dict.get(name).copied();

                stats.push(StrategyStats {
                    strategy_name: name.clone(),
                    alpha,
                    beta,
                    total_samples,
                    win_rate,
                    weight,
                    credible_interval: ci,
                });
            }

            Ok(stats)
        })
    }

    pub fn get_recommendation_sync(&self, name: &str) -> MLResult<RecommendationResponse> {
        Python::attach(|py| {
            let result = self
                .model
                .call_method1(py, "get_recommendation", (name,))
                .map_err(|e: PyErr| MLError::Other(format!("get_recommendation() failed: {e}")))?;

            let dict = result.bind(py);

            let use_strategy: bool = dict
                .get_item("use_strategy")
                .ok()
                .and_then(|v| v.extract::<bool>().ok())
                .unwrap_or(false);
            let reason: String = dict
                .get_item("reason")
                .ok()
                .and_then(|v| v.extract::<String>().ok())
                .unwrap_or_else(|| "unknown".into());
            let confidence: f64 = dict
                .get_item("confidence")
                .ok()
                .and_then(|v| v.extract::<f64>().ok())
                .unwrap_or(0.0);
            let expected_win_rate: Option<f64> = dict
                .get_item("expected_win_rate")
                .ok()
                .and_then(|v| v.extract::<f64>().ok());
            let credible_interval: Option<(f64, f64)> = dict
                .get_item("credible_interval")
                .ok()
                .and_then(|v| v.extract::<(f64, f64)>().ok());
            let samples: Option<i32> = dict
                .get_item("samples")
                .ok()
                .and_then(|v| v.extract::<i32>().ok());

            Ok(RecommendationResponse {
                use_strategy,
                reason,
                confidence,
                expected_win_rate,
                credible_interval,
                samples,
            })
        })
    }
}
