mod bayesian;
mod init;
mod price_predictor;
mod sentiment;
mod signal_models;

use async_trait::async_trait;
use ml_client::bayesian::{RecommendationResponse, StrategyStats};
use ml_client::error::{MLError, MLResult};
use ml_client::price_predictor::{DirectionPrediction, PriceData};
use ml_client::sentiment::{NewsSentimentResponse, SentimentResponse};
use ml_client::signal_models::{CalibrateResponse, EngineWeights, TradePrediction};
use ml_client::MLProvider;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use self::bayesian::EmbeddedBayesian;
use self::price_predictor::EmbeddedPricePredictor;
use self::sentiment::EmbeddedSentiment;
use self::signal_models::EmbeddedSignalModels;

/// All ML services running inside the Rust process via PyO3.
/// Each model is optional — only loaded if trained model files exist on disk.
pub struct EmbeddedMLEngine {
    signal_models: Option<Arc<EmbeddedSignalModels>>,
    sentiment: Option<Arc<EmbeddedSentiment>>,
    price_predictor: Option<Arc<EmbeddedPricePredictor>>,
    bayesian: Option<Arc<EmbeddedBayesian>>,
}

/// Check if a directory contains at least one file matching any of the given extensions.
fn dir_has_model_files(dir: &str, extensions: &[&str]) -> bool {
    let path = Path::new(dir);
    if !path.is_dir() {
        return false;
    }
    match std::fs::read_dir(path) {
        Ok(entries) => entries.filter_map(|e| e.ok()).any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            extensions.iter().any(|ext| name.ends_with(ext))
        }),
        Err(_) => false,
    }
}

impl EmbeddedMLEngine {
    /// Boot the embedded Python interpreter and load only trained models.
    ///
    /// A model is considered "trained and ready" if its directory contains
    /// the expected model files.  Models without files are skipped and their
    /// MLProvider methods return `ServiceUnavailable` (callers already handle
    /// this with graceful fallbacks).
    ///
    /// Env vars:
    /// - `ML_SERVICES_PATH` — root of `ml-services/` (default: `./ml-services`)
    /// - `ML_MODELS_PATH`   — root of model weights  (default: `./ml-services/models`)
    pub fn initialize() -> MLResult<Self> {
        // Prevent HuggingFace tokenizers and PyTorch from spawning child processes.
        // On macOS with PyO3, sys.executable = this Rust binary, so "spawn" method
        // would launch a new api-server instance causing an infinite loop.
        std::env::set_var("TOKENIZERS_PARALLELISM", "false");
        std::env::set_var("OMP_NUM_THREADS", "1");

        let ml_services_path =
            std::env::var("ML_SERVICES_PATH").unwrap_or_else(|_| "./ml-services".to_string());
        let ml_models_path = std::env::var("ML_MODELS_PATH")
            .unwrap_or_else(|_| format!("{}/models", ml_services_path));

        tracing::info!(
            "Initializing embedded ML engine (services={}, models={})",
            ml_services_path,
            ml_models_path
        );

        // Auto-create venv if VIRTUAL_ENV is set but doesn't exist
        if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
            let venv_path = std::path::Path::new(&venv);
            if !venv_path.join("bin").join("python").exists()
                && !venv_path.join("Scripts").join("python.exe").exists()
            {
                let setup_script = format!("{}/setup-venv.sh", ml_services_path);
                if std::path::Path::new(&setup_script).exists() {
                    tracing::info!("Venv not found at {}, running setup-venv.sh...", venv);
                    let status = std::process::Command::new("bash")
                        .arg(&setup_script)
                        .arg(&venv)
                        .status();
                    match status {
                        Ok(s) if s.success() => tracing::info!("Venv created successfully"),
                        Ok(s) => tracing::warn!("setup-venv.sh exited with {}", s),
                        Err(e) => tracing::warn!("Failed to run setup-venv.sh: {}", e),
                    }
                }
            }
        }

        // Configure Python interpreter paths
        init::setup_python(&ml_services_path)
            .map_err(|e| MLError::Other(format!("Python setup failed: {e}")))?;

        // Auto-detect best device: MPS (Apple Silicon) > CUDA (NVIDIA) > CPU
        let device = std::env::var("ML_DEVICE")
            .unwrap_or_else(|_| init::detect_device().unwrap_or_else(|| "cpu".to_string()));
        tracing::info!("ML device: {}", device);

        let mut loaded: Vec<&str> = Vec::new();
        let mut skipped: Vec<&str> = Vec::new();

        // --- Signal Models (MetaModel + Calibrator + WeightOptimizer) ---
        // Ready when: directory has .json (XGBoost) or .pkl (calibrator) files
        let signal_models_dir = format!("{}/signal_models", ml_models_path);
        let signal_models = if dir_has_model_files(&signal_models_dir, &[".json", ".pkl"]) {
            match EmbeddedSignalModels::load(&signal_models_dir) {
                Ok(m) => {
                    loaded.push("signal_models");
                    Some(Arc::new(m))
                }
                Err(e) => {
                    tracing::warn!("Signal models failed to load: {e}");
                    skipped.push("signal_models");
                    None
                }
            }
        } else {
            tracing::info!(
                "Signal models: no trained files in {}, skipping",
                signal_models_dir
            );
            skipped.push("signal_models");
            None
        };

        // --- FinBERT Sentiment ---
        // Ready when: HuggingFace cache directory has downloaded model files
        let sentiment_cache_dir = format!("{}/sentiment", ml_models_path);
        let sentiment = if dir_has_model_files(&sentiment_cache_dir, &[".safetensors", ".bin"])
            || dir_has_model_files(
                &format!("{}/models--ProsusAI--finbert", sentiment_cache_dir),
                &[".safetensors", ".bin"],
            )
            || has_hf_cache(&sentiment_cache_dir)
        {
            match EmbeddedSentiment::load(&sentiment_cache_dir, &device) {
                Ok(m) => {
                    loaded.push("sentiment");
                    Some(Arc::new(m))
                }
                Err(e) => {
                    tracing::warn!("Sentiment model failed to load: {e}");
                    skipped.push("sentiment");
                    None
                }
            }
        } else {
            tracing::info!(
                "Sentiment: no cached FinBERT in {}, skipping (run download first)",
                sentiment_cache_dir
            );
            skipped.push("sentiment");
            None
        };

        // --- Price Predictor ---
        // Ready when: directory has .pt (PyTorch checkpoint) file
        let price_predictor_dir = format!("{}/price_predictor", ml_models_path);
        let price_predictor = if dir_has_model_files(&price_predictor_dir, &[".pt", ".pth"]) {
            match EmbeddedPricePredictor::load(&price_predictor_dir, &device) {
                Ok(m) => {
                    loaded.push("price_predictor");
                    Some(Arc::new(m))
                }
                Err(e) => {
                    tracing::warn!("Price predictor failed to load: {e}");
                    skipped.push("price_predictor");
                    None
                }
            }
        } else {
            tracing::info!(
                "Price predictor: no checkpoint in {}, skipping",
                price_predictor_dir
            );
            skipped.push("price_predictor");
            None
        };

        // --- Bayesian Strategy Weights ---
        // Always loads — it's stateless with uniform priors and accumulates online.
        let bayesian = match EmbeddedBayesian::load() {
            Ok(b) => {
                loaded.push("bayesian");
                Some(Arc::new(b))
            }
            Err(e) => {
                tracing::warn!("Bayesian weights failed to init: {e}");
                skipped.push("bayesian");
                None
            }
        };

        if loaded.is_empty() {
            tracing::warn!("No ML models loaded — all methods will return fallback values");
        } else {
            tracing::info!(
                "Embedded ML engine ready: loaded=[{}], skipped=[{}]",
                loaded.join(", "),
                skipped.join(", ")
            );
        }

        Ok(Self {
            signal_models,
            sentiment,
            price_predictor,
            bayesian,
        })
    }
}

/// Check if a HuggingFace cache directory has any blob files (downloaded model).
fn has_hf_cache(cache_dir: &str) -> bool {
    let path = Path::new(cache_dir);
    if !path.is_dir() {
        return false;
    }
    // HF cache structure: models--ORG--NAME/blobs/ or snapshots/
    for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("models--") {
            let blobs = entry.path().join("blobs");
            if blobs.is_dir() {
                return std::fs::read_dir(&blobs)
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);
            }
        }
    }
    false
}

#[async_trait]
impl MLProvider for EmbeddedMLEngine {
    async fn predict_trade(&self, features: &HashMap<String, f64>) -> MLResult<TradePrediction> {
        let models = self
            .signal_models
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("signal models not loaded".into()))?
            .clone();
        let features = features.clone();
        tokio::task::spawn_blocking(move || models.predict_sync(&features))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn batch_calibrate(
        &self,
        engines: &HashMap<String, f64>,
        regime: &str,
    ) -> MLResult<HashMap<String, CalibrateResponse>> {
        let models = self
            .signal_models
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("signal models not loaded".into()))?
            .clone();
        let engines = engines.clone();
        let regime = regime.to_string();
        tokio::task::spawn_blocking(move || models.batch_calibrate_sync(&engines, &regime))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn get_optimal_weights(
        &self,
        features: &HashMap<String, f64>,
    ) -> MLResult<EngineWeights> {
        let models = self
            .signal_models
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("signal models not loaded".into()))?
            .clone();
        let features = features.clone();
        tokio::task::spawn_blocking(move || models.get_weights_sync(&features))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn predict_sentiment(&self, texts: Vec<String>) -> MLResult<SentimentResponse> {
        let model = self
            .sentiment
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("sentiment model not loaded".into()))?
            .clone();
        tokio::task::spawn_blocking(move || model.predict_sync(texts))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn analyze_news(
        &self,
        headlines: Vec<String>,
        descriptions: Option<Vec<String>>,
    ) -> MLResult<NewsSentimentResponse> {
        let model = self
            .sentiment
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("sentiment model not loaded".into()))?
            .clone();
        tokio::task::spawn_blocking(move || model.analyze_news_sync(headlines, descriptions))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn predict_price(
        &self,
        symbol: &str,
        history: Vec<PriceData>,
        horizon: i32,
    ) -> MLResult<DirectionPrediction> {
        let model = self
            .price_predictor
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("price predictor not loaded".into()))?
            .clone();
        let sym = symbol.to_string();
        tokio::task::spawn_blocking(move || model.predict_sync(&sym, &history, horizon))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn update_strategy(&self, name: &str, outcome: i32, pnl: Option<f64>) -> MLResult<()> {
        let model = self
            .bayesian
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("bayesian weights not loaded".into()))?
            .clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || model.update_strategy_sync(&name, outcome, pnl))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn get_strategy_weights(&self, normalize: bool) -> MLResult<HashMap<String, f64>> {
        let model = self
            .bayesian
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("bayesian weights not loaded".into()))?
            .clone();
        tokio::task::spawn_blocking(move || model.get_weights_sync(normalize))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn get_all_strategy_stats(&self) -> MLResult<Vec<StrategyStats>> {
        let model = self
            .bayesian
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("bayesian weights not loaded".into()))?
            .clone();
        tokio::task::spawn_blocking(move || model.get_all_stats_sync())
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    async fn get_recommendation(&self, name: &str) -> MLResult<RecommendationResponse> {
        let model = self
            .bayesian
            .as_ref()
            .ok_or_else(|| MLError::ServiceUnavailable("bayesian weights not loaded".into()))?
            .clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || model.get_recommendation_sync(&name))
            .await
            .map_err(|e| MLError::Other(e.to_string()))?
    }

    fn backend_name(&self) -> &'static str {
        "embedded-pyo3"
    }
}
