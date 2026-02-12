use ml_client::error::{MLError, MLResult};
use ml_client::sentiment::{NewsSentimentResponse, SentimentPrediction, SentimentResponse};
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::time::Instant;

/// In-process wrapper around the Python `sentiment.model.FinBERTSentiment` class.
pub struct EmbeddedSentiment {
    model: Py<PyAny>,
}

unsafe impl Send for EmbeddedSentiment {}
unsafe impl Sync for EmbeddedSentiment {}

impl EmbeddedSentiment {
    pub fn load(cache_dir: &str, device: &str) -> MLResult<Self> {
        Python::attach(|py| {
            let module = py.import("sentiment.model").map_err(|e: PyErr| {
                MLError::Other(format!("Failed to import sentiment.model: {e}"))
            })?;

            let model = module
                .getattr("FinBERTSentiment")
                .map_err(|e: PyErr| MLError::Other(format!("FinBERTSentiment not found: {e}")))?
                .call1(("ProsusAI/finbert", device, false, false, cache_dir))
                .map_err(|e: PyErr| MLError::Other(format!("FinBERTSentiment init failed: {e}")))?;

            Ok(Self {
                model: model.unbind(),
            })
        })
    }

    pub fn predict_sync(&self, texts: Vec<String>) -> MLResult<SentimentResponse> {
        Python::attach(|py| {
            let start = Instant::now();

            let py_texts = PyList::new(py, &texts)
                .map_err(|e: PyErr| MLError::Other(format!("Failed to create PyList: {e}")))?;

            let result = self
                .model
                .call_method1(py, "predict", (py_texts,))
                .map_err(|e: PyErr| {
                    MLError::Other(format!("FinBERTSentiment.predict() failed: {e}"))
                })?;

            let predictions_list: Vec<Py<PyAny>> =
                result.bind(py).extract().map_err(|e: PyErr| {
                    MLError::Other(format!("Failed to extract prediction list: {e}"))
                })?;

            let mut predictions = Vec::with_capacity(predictions_list.len());
            for pred_obj in &predictions_list {
                let pred = pred_obj.bind(py);
                let label: String = pred
                    .get_item("label")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let positive: f64 = pred
                    .get_item("positive")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let negative: f64 = pred
                    .get_item("negative")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let neutral: f64 = pred
                    .get_item("neutral")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let confidence: f64 = pred
                    .get_item("confidence")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
                let score: f64 = pred
                    .get_item("score")
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?
                    .extract()
                    .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

                predictions.push(SentimentPrediction {
                    label,
                    positive,
                    negative,
                    neutral,
                    confidence,
                    score,
                });
            }

            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

            Ok(SentimentResponse {
                predictions,
                processing_time_ms: elapsed_ms,
            })
        })
    }

    pub fn analyze_news_sync(
        &self,
        headlines: Vec<String>,
        descriptions: Option<Vec<String>>,
    ) -> MLResult<NewsSentimentResponse> {
        Python::attach(|py| {
            let start = Instant::now();

            let py_headlines = PyList::new(py, &headlines).map_err(|e: PyErr| {
                MLError::Other(format!("Failed to create headlines list: {e}"))
            })?;

            let py_descriptions = match &descriptions {
                Some(descs) => {
                    let list = PyList::new(py, descs).map_err(|e: PyErr| {
                        MLError::Other(format!("Failed to create descriptions list: {e}"))
                    })?;
                    list.into_any().unbind()
                }
                None => py.None(),
            };

            let result = self
                .model
                .call_method1(py, "analyze_news", (py_headlines, py_descriptions))
                .map_err(|e: PyErr| MLError::Other(format!("analyze_news() failed: {e}")))?;

            let dict = result.bind(py);

            let overall_sentiment: String = dict
                .get_item("overall_sentiment")
                .map_err(|e: PyErr| MLError::Other(format!("missing overall_sentiment: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let score: f64 = dict
                .get_item("score")
                .map_err(|e: PyErr| MLError::Other(format!("missing score: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let confidence: f64 = dict
                .get_item("confidence")
                .map_err(|e: PyErr| MLError::Other(format!("missing confidence: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let positive_ratio: f64 = dict
                .get_item("positive_ratio")
                .map_err(|e: PyErr| MLError::Other(format!("missing positive_ratio: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let negative_ratio: f64 = dict
                .get_item("negative_ratio")
                .map_err(|e: PyErr| MLError::Other(format!("missing negative_ratio: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let neutral_ratio: f64 = dict
                .get_item("neutral_ratio")
                .map_err(|e: PyErr| MLError::Other(format!("missing neutral_ratio: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;
            let article_count: usize = dict
                .get_item("article_count")
                .map_err(|e: PyErr| MLError::Other(format!("missing article_count: {e}")))?
                .extract()
                .map_err(|e: PyErr| MLError::Other(e.to_string()))?;

            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

            Ok(NewsSentimentResponse {
                overall_sentiment,
                score,
                confidence,
                positive_ratio,
                negative_ratio,
                neutral_ratio,
                article_count,
                processing_time_ms: elapsed_ms,
            })
        })
    }
}
