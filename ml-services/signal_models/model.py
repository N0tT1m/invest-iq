"""Model classes: MetaModel, ConfidenceCalibrator, WeightOptimizer."""
import json
import pickle
import numpy as np
from typing import Dict, Optional, Tuple
from pathlib import Path
import logging

logger = logging.getLogger(__name__)

try:
    import xgboost as xgb
    HAS_XGBOOST = True
except ImportError:
    HAS_XGBOOST = False
    logger.warning("xgboost not installed — models will use fallback predictions")

try:
    from sklearn.isotonic import IsotonicRegression
    HAS_SKLEARN = True
except ImportError:
    HAS_SKLEARN = False
    logger.warning("scikit-learn not installed — calibrator will use fallback")


class MetaModel:
    """XGBoost meta-model: predicts P(profitable) and expected return from 23 features.

    Two heads:
      - classifier: binary (profitable or not)
      - regressor: expected return percentage
    """

    def __init__(self, model_dir: str = "./models/signal_models"):
        self.model_dir = Path(model_dir)
        self.classifier: Optional[xgb.Booster] = None
        self.regressor: Optional[xgb.Booster] = None
        self._loaded = False

    def load(self) -> bool:
        """Load trained models from disk. Returns True if successful."""
        clf_path = self.model_dir / "meta_model_clf.json"
        reg_path = self.model_dir / "meta_model_reg.json"

        if not clf_path.exists() or not reg_path.exists():
            logger.info("Meta-model files not found at %s — using fallback", self.model_dir)
            return False

        if not HAS_XGBOOST:
            logger.warning("xgboost not installed — cannot load meta-model")
            return False

        try:
            self.classifier = xgb.Booster()
            self.classifier.load_model(str(clf_path))
            self.regressor = xgb.Booster()
            self.regressor.load_model(str(reg_path))
            self._loaded = True
            logger.info("Meta-model loaded from %s", self.model_dir)
            return True
        except Exception as e:
            logger.error("Failed to load meta-model: %s", e)
            self._loaded = False
            return False

    @property
    def is_loaded(self) -> bool:
        return self._loaded

    def predict(self, features: np.ndarray) -> Tuple[float, float, str]:
        """Predict trade outcome.

        Args:
            features: shape (23,) numpy array

        Returns:
            (probability, expected_return, recommendation)
            recommendation is "EXECUTE" if probability >= 0.6, else "SKIP"
        """
        if not self._loaded:
            return 0.5, 0.0, "SKIP"

        dmat = xgb.DMatrix(features.reshape(1, -1))
        probability = float(self.classifier.predict(dmat)[0])
        expected_return = float(self.regressor.predict(dmat)[0])

        recommendation = "EXECUTE" if probability >= 0.6 else "SKIP"
        return probability, expected_return, recommendation

    def save(self, classifier: "xgb.Booster", regressor: "xgb.Booster"):
        """Save trained models to disk."""
        self.model_dir.mkdir(parents=True, exist_ok=True)
        classifier.save_model(str(self.model_dir / "meta_model_clf.json"))
        regressor.save_model(str(self.model_dir / "meta_model_reg.json"))
        self.classifier = classifier
        self.regressor = regressor
        self._loaded = True
        logger.info("Meta-model saved to %s", self.model_dir)


class ConfidenceCalibrator:
    """Isotonic regression calibrator: maps raw confidence to calibrated probability.

    One model per engine (technical, fundamental, quantitative, sentiment).
    Input: raw_confidence (0..1), with optional signal_strength and market_regime as context.
    Output: calibrated confidence (0..1).
    """

    ENGINE_NAMES = ["technical", "fundamental", "quantitative", "sentiment"]

    def __init__(self, model_dir: str = "./models/signal_models"):
        self.model_dir = Path(model_dir)
        self.calibrators: Dict[str, IsotonicRegression] = {}
        self._loaded = False

    def load(self) -> bool:
        """Load calibrators from disk. Returns True if at least one loaded."""
        if not HAS_SKLEARN:
            logger.warning("scikit-learn not installed — cannot load calibrators")
            return False

        loaded_any = False
        for engine in self.ENGINE_NAMES:
            path = self.model_dir / f"calibrator_{engine}.pkl"
            if path.exists():
                try:
                    with open(path, "rb") as f:
                        self.calibrators[engine] = pickle.load(f)
                    loaded_any = True
                except Exception as e:
                    logger.error("Failed to load calibrator for %s: %s", engine, e)

        self._loaded = loaded_any
        if loaded_any:
            logger.info("Loaded calibrators for: %s", list(self.calibrators.keys()))
        return loaded_any

    @property
    def is_loaded(self) -> bool:
        return self._loaded

    def calibrate(self, engine: str, raw_confidence: float,
                  signal_strength: int = 0, market_regime: str = "normal") -> Tuple[float, str]:
        """Calibrate a raw confidence score.

        Args:
            engine: One of "technical", "fundamental", "quantitative", "sentiment"
            raw_confidence: Raw confidence from engine (0..1)
            signal_strength: Signal score (-100..100)
            market_regime: "low_volatility", "normal", "high_volatility"

        Returns:
            (calibrated_confidence, reliability_tier)
        """
        if engine not in self.calibrators:
            # Fallback: return raw confidence unchanged
            tier = _confidence_tier(raw_confidence)
            return raw_confidence, tier

        try:
            # Isotonic regression expects a 1D input (just the raw confidence)
            # We trained it on raw_confidence as X, actual accuracy as Y
            calibrated = float(self.calibrators[engine].predict([raw_confidence])[0])
            calibrated = max(0.0, min(1.0, calibrated))
            tier = _confidence_tier(calibrated)
            return calibrated, tier
        except Exception as e:
            logger.error("Calibration failed for %s: %s", engine, e)
            return raw_confidence, _confidence_tier(raw_confidence)

    def save(self, engine: str, model: "IsotonicRegression"):
        """Save a trained calibrator for an engine."""
        self.model_dir.mkdir(parents=True, exist_ok=True)
        path = self.model_dir / f"calibrator_{engine}.pkl"
        with open(path, "wb") as f:
            pickle.dump(model, f)
        self.calibrators[engine] = model
        self._loaded = True
        logger.info("Calibrator for %s saved to %s", engine, path)


class WeightOptimizer:
    """XGBoost regressor: predicts optimal engine weights given current conditions.

    Input: 23 features (same as meta-model)
    Output: 4 weights (technical, fundamental, quantitative, sentiment) summing to 1.0
    """

    DEFAULT_WEIGHTS = {
        "technical": 0.20,
        "fundamental": 0.40,
        "quantitative": 0.15,
        "sentiment": 0.25,
    }

    def __init__(self, model_dir: str = "./models/signal_models"):
        self.model_dir = Path(model_dir)
        self.model: Optional[xgb.Booster] = None
        self._loaded = False

    def load(self) -> bool:
        """Load trained model from disk."""
        path = self.model_dir / "weight_optimizer.json"
        if not path.exists():
            logger.info("Weight optimizer not found at %s — using default weights", path)
            return False

        if not HAS_XGBOOST:
            logger.warning("xgboost not installed — cannot load weight optimizer")
            return False

        try:
            self.model = xgb.Booster()
            self.model.load_model(str(path))
            self._loaded = True
            logger.info("Weight optimizer loaded from %s", path)
            return True
        except Exception as e:
            logger.error("Failed to load weight optimizer: %s", e)
            return False

    @property
    def is_loaded(self) -> bool:
        return self._loaded

    def predict_weights(self, features: np.ndarray) -> Dict[str, float]:
        """Predict optimal engine weights.

        Args:
            features: shape (23,) numpy array

        Returns:
            Dict with keys technical, fundamental, quantitative, sentiment.
            Values sum to 1.0.
        """
        if not self._loaded:
            return dict(self.DEFAULT_WEIGHTS)

        try:
            dmat = xgb.DMatrix(features.reshape(1, -1))
            raw = self.model.predict(dmat)[0]  # shape (4,)

            # Softmax normalization to ensure positive values summing to 1
            exp_vals = np.exp(raw - np.max(raw))  # numerical stability
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

    def save(self, model: "xgb.Booster"):
        """Save trained model to disk."""
        self.model_dir.mkdir(parents=True, exist_ok=True)
        model.save_model(str(self.model_dir / "weight_optimizer.json"))
        self.model = model
        self._loaded = True
        logger.info("Weight optimizer saved to %s", self.model_dir)


def _confidence_tier(confidence: float) -> str:
    """Map confidence to a human-readable tier."""
    if confidence >= 0.8:
        return "high"
    elif confidence >= 0.6:
        return "moderate"
    elif confidence >= 0.4:
        return "low"
    else:
        return "very_low"
