use ml_client::error::{MLError, MLResult};
use ml_client::signal_models::{CalibrateResponse, EngineWeights, TradePrediction};
use pyo3::prelude::*;
use std::collections::HashMap;

/// In-process wrapper around the Python `signal_models.model` module.
pub struct EmbeddedSignalModels {
    meta_model: Py<PyAny>,
    calibrator: Py<PyAny>,
    weight_optimizer: Py<PyAny>,
}

unsafe impl Send for EmbeddedSignalModels {}
unsafe impl Sync for EmbeddedSignalModels {}

impl EmbeddedSignalModels {
    pub fn load(model_dir: &str) -> MLResult<Self> {
        Python::attach(|py| {
            let module = py.import("signal_models.model").map_err(|e: PyErr| {
                MLError::Other(format!("Failed to import signal_models.model: {e}"))
            })?;

            let meta_model = module
                .getattr("MetaModel")
                .map_err(|e: PyErr| MLError::Other(format!("MetaModel class not found: {e}")))?
                .call1((model_dir,))
                .map_err(|e: PyErr| MLError::Other(format!("MetaModel init failed: {e}")))?;
            meta_model
                .call_method0("load")
                .map_err(|e: PyErr| MLError::Other(format!("MetaModel.load() failed: {e}")))?;

            let calibrator = module
                .getattr("ConfidenceCalibrator")
                .map_err(|e: PyErr| {
                    MLError::Other(format!("ConfidenceCalibrator class not found: {e}"))
                })?
                .call1((model_dir,))
                .map_err(|e: PyErr| {
                    MLError::Other(format!("ConfidenceCalibrator init failed: {e}"))
                })?;
            calibrator.call_method0("load").map_err(|e: PyErr| {
                MLError::Other(format!("ConfidenceCalibrator.load() failed: {e}"))
            })?;

            let weight_optimizer = module
                .getattr("WeightOptimizer")
                .map_err(|e: PyErr| {
                    MLError::Other(format!("WeightOptimizer class not found: {e}"))
                })?
                .call1((model_dir,))
                .map_err(|e: PyErr| MLError::Other(format!("WeightOptimizer init failed: {e}")))?;
            weight_optimizer.call_method0("load").map_err(|e: PyErr| {
                MLError::Other(format!("WeightOptimizer.load() failed: {e}"))
            })?;

            Ok(Self {
                meta_model: meta_model.unbind(),
                calibrator: calibrator.unbind(),
                weight_optimizer: weight_optimizer.unbind(),
            })
        })
    }

    pub fn predict_sync(&self, features: &HashMap<String, f64>) -> MLResult<TradePrediction> {
        Python::attach(|py| {
            let features_module = py.import("signal_models.features").map_err(|e: PyErr| {
                MLError::Other(format!("Failed to import features module: {e}"))
            })?;
            let feature_names: Vec<String> = features_module
                .getattr("FEATURE_NAMES")
                .map_err(|e: PyErr| MLError::Other(format!("FEATURE_NAMES not found: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract FEATURE_NAMES: {e}"))
                })?;

            let np = py
                .import("numpy")
                .map_err(|e: PyErr| MLError::Other(format!("numpy not found: {e}")))?;

            let values: Vec<f64> = feature_names
                .iter()
                .map(|name: &String| *features.get(name.as_str()).unwrap_or(&0.0))
                .collect();

            let arr = np
                .call_method1("array", (values,))
                .map_err(|e: PyErr| MLError::Other(format!("numpy.array() failed: {e}")))?;

            let result = self
                .meta_model
                .call_method1(py, "predict", (arr,))
                .map_err(|e: PyErr| MLError::Other(format!("MetaModel.predict() failed: {e}")))?;

            let tuple = result.bind(py);
            let probability: f64 = tuple
                .get_item(0)
                .map_err(|e: PyErr| MLError::Other(format!("Failed to get probability: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract probability: {e}"))
                })?;
            let expected_return: f64 = tuple
                .get_item(1)
                .map_err(|e: PyErr| MLError::Other(format!("Failed to get expected_return: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract expected_return: {e}"))
                })?;
            let recommendation: String = tuple
                .get_item(2)
                .map_err(|e: PyErr| MLError::Other(format!("Failed to get recommendation: {e}")))?
                .extract()
                .map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract recommendation: {e}"))
                })?;

            Ok(TradePrediction {
                probability,
                expected_return,
                recommendation,
            })
        })
    }

    pub fn batch_calibrate_sync(
        &self,
        engines: &HashMap<String, f64>,
        regime: &str,
    ) -> MLResult<HashMap<String, CalibrateResponse>> {
        Python::attach(|py| {
            let mut results = HashMap::new();

            for (engine, raw_confidence) in engines {
                let result = self
                    .calibrator
                    .call_method1(
                        py,
                        "calibrate",
                        (engine.as_str(), *raw_confidence, 0i32, regime),
                    )
                    .map_err(|e: PyErr| {
                        MLError::Other(format!(
                            "ConfidenceCalibrator.calibrate({engine}) failed: {e}"
                        ))
                    })?;

                let tuple = result.bind(py);
                let calibrated_confidence: f64 = tuple
                    .get_item(0)
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let reliability_tier: String = tuple
                    .get_item(1)
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

                results.insert(
                    engine.clone(),
                    CalibrateResponse {
                        calibrated_confidence,
                        reliability_tier,
                    },
                );
            }

            Ok(results)
        })
    }

    pub fn get_weights_sync(&self, features: &HashMap<String, f64>) -> MLResult<EngineWeights> {
        Python::attach(|py| {
            let features_module = py
                .import("signal_models.features")
                .map_err(|e: PyErr| MLError::Other(format!("Failed to import features: {e}")))?;
            let feature_names: Vec<String> = features_module
                .getattr("FEATURE_NAMES")
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            let np = py
                .import("numpy")
                .map_err(|e: PyErr| MLError::Other(format!("numpy not found: {e}")))?;

            let values: Vec<f64> = feature_names
                .iter()
                .map(|name: &String| *features.get(name.as_str()).unwrap_or(&0.0))
                .collect();

            let arr = np
                .call_method1("array", (values,))
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            let result = self
                .weight_optimizer
                .call_method1(py, "predict_weights", (arr,))
                .map_err(|e: PyErr| {
                    MLError::Other(format!("WeightOptimizer.predict_weights() failed: {e}"))
                })?;

            let dict: HashMap<String, f64> = result.bind(py).extract().map_err(|e: PyErr| {
                MLError::Other(format!("Failed to extract weights dict: {e}"))
            })?;

            Ok(EngineWeights { weights: dict })
        })
    }
}
