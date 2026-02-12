"""FastAPI service for Signal Models: Meta-Model, Calibrator, Weight Optimizer.

Port 8004. Provides /predict, /calibrate, /batch-calibrate, /weights, /health.
"""
import logging
import time
import numpy as np
from typing import Dict, Optional
from pathlib import Path

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
import uvicorn

from .model import MetaModel, ConfidenceCalibrator, WeightOptimizer
from .features import FEATURE_NAMES, features_to_array

logger = logging.getLogger(__name__)

app = FastAPI(title="Signal Models Service", version="1.0.0")

# Production hardening middleware
from shared.middleware import setup_hardening
setup_hardening(app, "signal-models")

# Global model instances
meta_model: Optional[MetaModel] = None
calibrator: Optional[ConfidenceCalibrator] = None
weight_optimizer: Optional[WeightOptimizer] = None

# Track load state
models_loaded = {
    "meta_model": False,
    "calibrator": False,
    "weight_optimizer": False,
}

MODEL_DIR = "./models/signal_models"


# ---- Request/Response Models ----

class PredictRequest(BaseModel):
    features: Dict[str, float] = Field(..., description="23-feature vector as key-value pairs")


class PredictResponse(BaseModel):
    probability: float
    expected_return: float
    recommendation: str  # "EXECUTE" or "SKIP"


class CalibrateRequest(BaseModel):
    engine: str = Field(..., description="Engine name: technical, fundamental, quantitative, sentiment")
    raw_confidence: float = Field(..., ge=0.0, le=1.0)
    signal_strength: int = Field(default=0, ge=-100, le=100)
    market_regime: str = Field(default="normal")


class CalibrateResponse(BaseModel):
    calibrated_confidence: float
    reliability_tier: str


class BatchCalibrateRequest(BaseModel):
    engines: Dict[str, float] = Field(
        ...,
        description="Engine name to raw confidence mapping",
        examples=[{"technical": 0.8, "fundamental": 0.7, "quantitative": 0.6, "sentiment": 0.5}],
    )
    market_regime: str = Field(default="normal")


class BatchCalibrateResponse(BaseModel):
    calibrations: Dict[str, CalibrateResponse]


class WeightsRequest(BaseModel):
    features: Dict[str, float] = Field(..., description="23-feature vector as key-value pairs")


class WeightsResponse(BaseModel):
    weights: Dict[str, float]


class HealthResponse(BaseModel):
    status: str
    models: Dict[str, bool]
    feature_count: int
    model_dir: str


class ModelInfoResponse(BaseModel):
    model_dir: str
    models_loaded: Dict[str, bool]
    training_date: Optional[str] = None
    days_since_training: Optional[int] = None
    feature_count: int


# ---- Startup/Shutdown ----

@app.on_event("startup")
async def startup():
    global meta_model, calibrator, weight_optimizer

    logger.info("Loading signal models from %s...", MODEL_DIR)

    meta_model = MetaModel(MODEL_DIR)
    models_loaded["meta_model"] = meta_model.load()

    calibrator = ConfidenceCalibrator(MODEL_DIR)
    models_loaded["calibrator"] = calibrator.load()

    weight_optimizer = _load_weight_optimizer(MODEL_DIR)
    models_loaded["weight_optimizer"] = weight_optimizer is not None

    loaded = sum(1 for v in models_loaded.values() if v)
    logger.info("Signal models startup complete: %d/3 models loaded", loaded)
    if not any(models_loaded.values()):
        logger.info("No trained models found — service will return fallback predictions")


def _load_weight_optimizer(model_dir: str) -> Optional[WeightOptimizer]:
    """Load weight optimizer (multi-model variant)."""
    import json as _json

    manifest_path = Path(model_dir) / "weight_optimizer_manifest.json"
    if not manifest_path.exists():
        # Try single-file format
        wo = WeightOptimizer(model_dir)
        if wo.load():
            return wo
        return None

    try:
        import xgboost as xgb
    except ImportError:
        logger.warning("xgboost not installed — cannot load weight optimizer")
        return None

    try:
        with open(manifest_path) as f:
            manifest = _json.load(f)

        n_models = manifest.get("n_models", 4)
        models = []
        for i in range(n_models):
            path = Path(model_dir) / f"weight_optimizer_{i}.json"
            if not path.exists():
                return None
            m = xgb.Booster()
            m.load_model(str(path))
            models.append(m)

        # Return a wrapper that uses multi-model prediction
        wo = _MultiWeightOptimizer(models, model_dir)
        logger.info("Weight optimizer loaded (%d sub-models)", n_models)
        return wo
    except Exception as e:
        logger.error("Failed to load weight optimizer: %s", e)
        return None


class _MultiWeightOptimizer:
    """Wrapper around 4 XGBoost models for multi-output weight prediction."""

    DEFAULT_WEIGHTS = {
        "technical": 0.20,
        "fundamental": 0.40,
        "quantitative": 0.15,
        "sentiment": 0.25,
    }

    def __init__(self, models, model_dir: str):
        self.models = models
        self.model_dir = model_dir
        self._loaded = True

    @property
    def is_loaded(self) -> bool:
        return self._loaded

    def predict_weights(self, features: np.ndarray) -> Dict[str, float]:
        try:
            import xgboost as xgb
            dmat = xgb.DMatrix(features.reshape(1, -1), feature_names=FEATURE_NAMES)
            raw = np.array([m.predict(dmat)[0] for m in self.models])

            # Softmax normalize
            exp_vals = np.exp(raw - np.max(raw))
            weights = exp_vals / exp_vals.sum()

            return {
                "technical": float(weights[0]),
                "fundamental": float(weights[1]),
                "quantitative": float(weights[2]),
                "sentiment": float(weights[3]),
            }
        except Exception as e:
            logger.error("Weight prediction failed: %s", e)
            return dict(self.DEFAULT_WEIGHTS)


# ---- Endpoints ----

@app.get("/health", response_model=HealthResponse)
async def health():
    return HealthResponse(
        status="healthy",
        models=models_loaded,
        feature_count=len(FEATURE_NAMES),
        model_dir=MODEL_DIR,
    )


@app.get("/model-info", response_model=ModelInfoResponse)
async def model_info():
    """Return model metadata including training date and staleness."""
    import os
    from datetime import datetime

    training_date = None
    days_since = None

    # Check meta model file modification time as proxy for training date
    meta_path = Path(MODEL_DIR) / "meta_model.json"
    if meta_path.exists():
        mtime = os.path.getmtime(str(meta_path))
        dt = datetime.fromtimestamp(mtime)
        training_date = dt.isoformat()
        days_since = (datetime.now() - dt).days

    return ModelInfoResponse(
        model_dir=MODEL_DIR,
        models_loaded=models_loaded,
        training_date=training_date,
        days_since_training=days_since,
        feature_count=len(FEATURE_NAMES),
    )


@app.post("/predict", response_model=PredictResponse)
async def predict(request: PredictRequest):
    """Meta-model prediction: should we take this trade?"""
    start = time.time()

    features_arr = features_to_array(request.features)

    if meta_model and meta_model.is_loaded:
        probability, expected_return, recommendation = meta_model.predict(features_arr)
    else:
        # Fallback: no trained model
        probability = 0.5
        expected_return = 0.0
        recommendation = "SKIP"

    elapsed = (time.time() - start) * 1000
    logger.debug("Predict: %.1fms, prob=%.3f, rec=%s", elapsed, probability, recommendation)

    return PredictResponse(
        probability=round(probability, 4),
        expected_return=round(expected_return, 4),
        recommendation=recommendation,
    )


@app.post("/calibrate", response_model=CalibrateResponse)
async def calibrate(request: CalibrateRequest):
    """Calibrate a raw confidence score for a specific engine."""
    if request.engine not in ConfidenceCalibrator.ENGINE_NAMES:
        raise HTTPException(
            status_code=400,
            detail=f"Unknown engine: {request.engine}. Must be one of {ConfidenceCalibrator.ENGINE_NAMES}",
        )

    if calibrator and calibrator.is_loaded:
        cal_conf, tier = calibrator.calibrate(
            request.engine, request.raw_confidence,
            request.signal_strength, request.market_regime,
        )
    else:
        # Fallback: return raw confidence unchanged
        cal_conf = request.raw_confidence
        tier = _fallback_tier(request.raw_confidence)

    return CalibrateResponse(
        calibrated_confidence=round(cal_conf, 4),
        reliability_tier=tier,
    )


@app.post("/batch-calibrate", response_model=BatchCalibrateResponse)
async def batch_calibrate(request: BatchCalibrateRequest):
    """Calibrate all engines at once."""
    results = {}
    for engine, raw_conf in request.engines.items():
        if engine not in ConfidenceCalibrator.ENGINE_NAMES:
            results[engine] = CalibrateResponse(
                calibrated_confidence=raw_conf,
                reliability_tier=_fallback_tier(raw_conf),
            )
            continue

        if calibrator and calibrator.is_loaded:
            cal_conf, tier = calibrator.calibrate(engine, raw_conf, market_regime=request.market_regime)
        else:
            cal_conf = raw_conf
            tier = _fallback_tier(raw_conf)

        results[engine] = CalibrateResponse(
            calibrated_confidence=round(cal_conf, 4),
            reliability_tier=tier,
        )

    return BatchCalibrateResponse(calibrations=results)


@app.post("/weights", response_model=WeightsResponse)
async def get_weights(request: WeightsRequest):
    """Get optimal engine weights for current conditions."""
    features_arr = features_to_array(request.features)

    if weight_optimizer and weight_optimizer.is_loaded:
        weights = weight_optimizer.predict_weights(features_arr)
    else:
        # Fallback: return default hardcoded weights
        weights = {
            "technical": 0.20,
            "fundamental": 0.40,
            "quantitative": 0.15,
            "sentiment": 0.25,
        }

    return WeightsResponse(weights=weights)


def _fallback_tier(conf: float) -> str:
    if conf >= 0.8:
        return "high"
    elif conf >= 0.6:
        return "moderate"
    elif conf >= 0.4:
        return "low"
    return "very_low"


def main():
    """Run the service."""
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s - %(message)s")
    uvicorn.run(
        "signal_models.service:app",
        host="0.0.0.0",
        port=8004,
        workers=1,
        log_level="info",
    )


if __name__ == "__main__":
    main()
