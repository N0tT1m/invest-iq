"""Training pipeline: load data, train models, save artifacts."""
from dotenv import load_dotenv
load_dotenv()
load_dotenv(dotenv_path="../.env")
import argparse
import json
import logging
import sys
import time
import numpy as np
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Tuple, Any

try:
    from .features import (
        FEATURE_NAMES, NUM_FEATURES, features_to_array, load_analysis_features,
        load_backtest_trades, load_training_from_bars, get_db_connection,
    )
    from .model import MetaModel, ConfidenceCalibrator, WeightOptimizer
except ImportError:
    from features import (
        FEATURE_NAMES, NUM_FEATURES, features_to_array, load_analysis_features,
        load_backtest_trades, load_training_from_bars, get_db_connection,
    )
    from model import MetaModel, ConfidenceCalibrator, WeightOptimizer

logger = logging.getLogger(__name__)

try:
    import xgboost as xgb
    HAS_XGBOOST = True
except ImportError:
    HAS_XGBOOST = False

try:
    from sklearn.isotonic import IsotonicRegression
    from sklearn.metrics import (
        accuracy_score, roc_auc_score, mean_absolute_error, f1_score,
    )
    HAS_SKLEARN = True
except ImportError:
    HAS_SKLEARN = False


def _time_split(data: List[Dict], train_ratio: float = 0.8) -> Tuple[List[Dict], List[Dict]]:
    """Time-based train/val split (no look-ahead)."""
    split_idx = int(len(data) * train_ratio)
    return data[:split_idx], data[split_idx:]


def prepare_meta_model_data(
    db_path: str,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Prepare training data for the meta-model.

    Returns:
        X: feature matrix (N, 23)
        y_cls: binary labels (1 = profitable)
        y_reg: return percentages
    """
    # Try analysis_features table first (production data)
    analysis_data = load_analysis_features(db_path)

    # Also try backtest trades
    backtest_data = load_backtest_trades(db_path)

    X_list = []
    y_cls_list = []
    y_reg_list = []

    # From analysis_features (has full feature vectors)
    for item in analysis_data:
        features = item["features"]
        arr = features_to_array(features)
        return_5d = item.get("actual_return_5d", 0.0) or 0.0
        X_list.append(arr)
        y_cls_list.append(1.0 if return_5d > 0 else 0.0)
        y_reg_list.append(return_5d)

    # From backtest trades (partial features — use signal & confidence only)
    for trade in backtest_data:
        pnl = trade.get("pnl_pct", 0.0) or 0.0
        confidence = trade.get("confidence", 0.5) or 0.5
        signal_type = trade.get("signal_type", "Neutral")

        signal_map = {
            "StrongBuy": 100, "Buy": 60, "WeakBuy": 30,
            "Neutral": 0,
            "WeakSell": -30, "Sell": -60, "StrongSell": -100,
        }
        signal_score = float(signal_map.get(signal_type, 0))

        # Build partial feature vector (zero-fill missing)
        features = np.zeros(NUM_FEATURES, dtype=np.float32)
        features[0] = signal_score  # technical_score as proxy
        features[4] = confidence    # technical_confidence as proxy
        X_list.append(features)
        y_cls_list.append(1.0 if pnl > 0 else 0.0)
        y_reg_list.append(pnl)

    # From training_bars (bulk bars from data-loader → derived features)
    X_bars, y_cls_bars, y_reg_bars = load_training_from_bars(db_path)
    if len(X_bars) > 0:
        logger.info("Adding %d samples from training_bars", len(X_bars))
        X_list.extend(X_bars.tolist())
        y_cls_list.extend(y_cls_bars.tolist())
        y_reg_list.extend(y_reg_bars.tolist())

    if not X_list:
        return np.empty((0, NUM_FEATURES)), np.empty(0), np.empty(0)

    X = np.array(X_list, dtype=np.float32)
    y_cls = np.array(y_cls_list, dtype=np.float32)
    y_reg = np.array(y_reg_list, dtype=np.float32)

    # --- Data filtering ---
    initial_len = len(X)

    # Remove rows with NaN/inf in features
    valid_mask = np.all(np.isfinite(X), axis=1)
    # Remove extreme return outliers (>50% move — likely data errors)
    valid_mask &= (y_reg > -50) & (y_reg < 50)
    # Remove rows where all features are zero (empty/failed analysis)
    valid_mask &= np.any(X != 0, axis=1)

    X, y_cls, y_reg = X[valid_mask], y_cls[valid_mask], y_reg[valid_mask]

    removed = initial_len - len(X)
    if removed > 0:
        logger.info("Meta-model data filtering: %d → %d samples (%d removed)",
                     initial_len, len(X), removed)

    return X, y_cls, y_reg


def train_meta_model(
    X: np.ndarray, y_cls: np.ndarray, y_reg: np.ndarray,
    output_dir: str,
) -> Dict[str, float]:
    """Train XGBoost meta-model (classifier + regressor).

    Returns metrics dict.
    """
    if not HAS_XGBOOST:
        raise RuntimeError("xgboost is required for training. Install with: pip install xgboost")

    # Time-based split
    n = len(X)
    split = int(n * 0.8)
    X_train, X_val = X[:split], X[split:]
    y_cls_train, y_cls_val = y_cls[:split], y_cls[split:]
    y_reg_train, y_reg_val = y_reg[:split], y_reg[split:]

    # --- Classifier ---
    dtrain_cls = xgb.DMatrix(X_train, label=y_cls_train, feature_names=FEATURE_NAMES)
    dval_cls = xgb.DMatrix(X_val, label=y_cls_val, feature_names=FEATURE_NAMES)

    # Use GPU if available (XGBoost supports cuda, not MPS)
    try:
        import torch
        xgb_device = "cuda" if torch.cuda.is_available() else "cpu"
    except ImportError:
        xgb_device = "cpu"

    clf_params = {
        "objective": "binary:logistic",
        "eval_metric": "auc",
        "max_depth": 5,
        "learning_rate": 0.05,
        "subsample": 0.8,
        "colsample_bytree": 0.8,
        "min_child_weight": 5,
        "device": xgb_device,
        "tree_method": "gpu_hist" if xgb_device == "cuda" else "hist",
        "seed": 42,
    }
    clf_model = xgb.train(
        clf_params, dtrain_cls,
        num_boost_round=200,
        evals=[(dval_cls, "val")],
        early_stopping_rounds=20,
        verbose_eval=False,
    )

    # --- Regressor ---
    dtrain_reg = xgb.DMatrix(X_train, label=y_reg_train, feature_names=FEATURE_NAMES)
    dval_reg = xgb.DMatrix(X_val, label=y_reg_val, feature_names=FEATURE_NAMES)

    reg_params = {
        "objective": "reg:squarederror",
        "eval_metric": "mae",
        "max_depth": 5,
        "learning_rate": 0.05,
        "subsample": 0.8,
        "colsample_bytree": 0.8,
        "min_child_weight": 5,
        "device": xgb_device,
        "tree_method": "gpu_hist" if xgb_device == "cuda" else "hist",
        "seed": 42,
    }
    reg_model = xgb.train(
        reg_params, dtrain_reg,
        num_boost_round=200,
        evals=[(dval_reg, "val")],
        early_stopping_rounds=20,
        verbose_eval=False,
    )

    # Evaluate
    pred_cls = clf_model.predict(dval_cls)
    pred_reg = reg_model.predict(dval_reg)

    metrics = {}
    if len(y_cls_val) > 0:
        pred_labels = (pred_cls >= 0.5).astype(int)
        if HAS_SKLEARN:
            metrics["accuracy"] = float(accuracy_score(y_cls_val, pred_labels))
            metrics["f1"] = float(f1_score(y_cls_val, pred_labels, zero_division=0))
            try:
                metrics["auc"] = float(roc_auc_score(y_cls_val, pred_cls))
            except ValueError:
                metrics["auc"] = 0.5
            metrics["mae"] = float(mean_absolute_error(y_reg_val, pred_reg))
        else:
            metrics["accuracy"] = float(np.mean(pred_labels == y_cls_val.astype(int)))
            metrics["mae"] = float(np.mean(np.abs(y_reg_val - pred_reg)))

    # Save
    meta = MetaModel(output_dir)
    meta.save(clf_model, reg_model)

    logger.info("Meta-model metrics: %s", metrics)
    return metrics


def prepare_calibration_data(
    db_path: str,
) -> Dict[str, Tuple[np.ndarray, np.ndarray]]:
    """Prepare calibration data per engine.

    Returns dict mapping engine name to (raw_confidences, actual_accuracy).
    """
    analysis_data = load_analysis_features(db_path)
    if not analysis_data:
        return {}

    engine_data: Dict[str, Tuple[List[float], List[float]]] = {
        "technical": ([], []),
        "fundamental": ([], []),
        "quantitative": ([], []),
        "sentiment": ([], []),
    }

    for item in analysis_data:
        features = item["features"]
        return_5d = item.get("actual_return_5d", 0.0) or 0.0
        actual_profitable = 1.0 if return_5d > 0 else 0.0

        for engine, conf_key in [
            ("technical", "technical_confidence"),
            ("fundamental", "fundamental_confidence"),
            ("quantitative", "quant_confidence"),
            ("sentiment", "sentiment_confidence"),
        ]:
            conf = features.get(conf_key, 0.0)
            if conf > 0:
                engine_data[engine][0].append(conf)
                engine_data[engine][1].append(actual_profitable)

    result = {}
    for engine, (confs, actuals) in engine_data.items():
        if len(confs) >= 10:
            result[engine] = (np.array(confs), np.array(actuals))
    return result


def train_calibrators(
    calibration_data: Dict[str, Tuple[np.ndarray, np.ndarray]],
    output_dir: str,
) -> Dict[str, Dict[str, float]]:
    """Train isotonic regression calibrators per engine.

    Returns metrics per engine.
    """
    if not HAS_SKLEARN:
        raise RuntimeError("scikit-learn is required for training. Install with: pip install scikit-learn")

    calibrator = ConfidenceCalibrator(output_dir)
    all_metrics = {}

    for engine, (raw_confs, actuals) in calibration_data.items():
        n = len(raw_confs)
        split = int(n * 0.8)

        X_train, X_val = raw_confs[:split], raw_confs[split:]
        y_train, y_val = actuals[:split], actuals[split:]

        iso = IsotonicRegression(y_min=0.0, y_max=1.0, out_of_bounds="clip")
        iso.fit(X_train, y_train)

        calibrator.save(engine, iso)

        # Evaluate
        if len(X_val) > 0:
            preds = iso.predict(X_val)
            mae = float(mean_absolute_error(y_val, preds))
            all_metrics[engine] = {"mae": mae, "n_samples": n}
            logger.info("Calibrator %s: MAE=%.4f, samples=%d", engine, mae, n)
        else:
            all_metrics[engine] = {"mae": 0.0, "n_samples": n}

    return all_metrics


def prepare_weight_data(
    db_path: str,
) -> Tuple[np.ndarray, np.ndarray]:
    """Prepare training data for weight optimizer.

    For each analysis with known outcome, determine which engine was most accurate
    and build target weights accordingly.

    Returns:
        X: feature matrix (N, 23)
        y: target weights (N, 4) — soft labels
    """
    analysis_data = load_analysis_features(db_path)
    if not analysis_data:
        return np.empty((0, NUM_FEATURES)), np.empty((0, 4))

    X_list = []
    y_list = []

    for item in analysis_data:
        features = item["features"]
        return_5d = item.get("actual_return_5d", 0.0) or 0.0
        arr = features_to_array(features)

        # Determine actual direction
        actual_direction = 1.0 if return_5d > 0 else (-1.0 if return_5d < 0 else 0.0)

        # Each engine's signal direction (normalized to -1..1)
        engine_scores = [
            features.get("technical_score", 0.0) / 100.0,
            features.get("fundamental_score", 0.0) / 100.0,
            features.get("quant_score", 0.0) / 100.0,
            features.get("sentiment_score", 0.0) / 100.0,
        ]

        # Engine accuracy: how close was each engine's prediction to reality?
        # Higher = better alignment with actual outcome
        engine_accuracy = []
        for score in engine_scores:
            if actual_direction != 0.0:
                # Agreement: both positive or both negative
                alignment = score * actual_direction  # positive if same direction
                engine_accuracy.append(max(0.0, alignment))
            else:
                # Actual was neutral — reward engines close to 0
                engine_accuracy.append(max(0.0, 1.0 - abs(score)))

        # Convert to soft weights via softmax
        acc_arr = np.array(engine_accuracy)
        if acc_arr.sum() > 0:
            # Temperature-scaled softmax for smoother weights
            exp_vals = np.exp(acc_arr * 3.0)  # temperature = 3
            weights = exp_vals / exp_vals.sum()
        else:
            weights = np.array([0.25, 0.25, 0.25, 0.25])

        X_list.append(arr)
        y_list.append(weights)

    # Also generate weight targets from training_bars if analysis data is sparse
    X_bars, y_cls_bars, y_reg_bars = load_training_from_bars(db_path)
    if len(X_bars) > 0:
        logger.info("Adding %d weight samples from training_bars", len(X_bars))
        for i in range(len(X_bars)):
            actual_direction = 1.0 if y_reg_bars[i] > 0 else (-1.0 if y_reg_bars[i] < 0 else 0.0)
            features = X_bars[i]
            # Only technical and quant scores are populated from bars
            engine_scores = [
                features[0] / 100.0,  # technical_score
                features[1] / 100.0,  # fundamental_score (0)
                features[2] / 100.0,  # quant_score
                features[3] / 100.0,  # sentiment_score (0)
            ]
            engine_accuracy = []
            for score in engine_scores:
                if actual_direction != 0.0:
                    alignment = score * actual_direction
                    engine_accuracy.append(max(0.0, alignment))
                else:
                    engine_accuracy.append(max(0.0, 1.0 - abs(score)))
            acc_arr = np.array(engine_accuracy)
            if acc_arr.sum() > 0:
                exp_vals = np.exp(acc_arr * 3.0)
                weights = exp_vals / exp_vals.sum()
            else:
                weights = np.array([0.25, 0.25, 0.25, 0.25])
            X_list.append(features)
            y_list.append(weights)

    X = np.array(X_list, dtype=np.float32)
    y = np.array(y_list, dtype=np.float32)

    # --- Data filtering ---
    initial_len = len(X)

    # Remove rows with NaN/inf
    valid_mask = np.all(np.isfinite(X), axis=1) & np.all(np.isfinite(y), axis=1)
    # Remove rows where all features are zero
    valid_mask &= np.any(X != 0, axis=1)

    X, y = X[valid_mask], y[valid_mask]

    removed = initial_len - len(X)
    if removed > 0:
        logger.info("Weight optimizer data filtering: %d → %d samples (%d removed)",
                     initial_len, len(X), removed)

    return X, y


def train_weight_optimizer(
    X: np.ndarray, y: np.ndarray,
    output_dir: str,
) -> Dict[str, float]:
    """Train XGBoost multi-output regressor for weight optimization.

    Returns metrics dict.
    """
    if not HAS_XGBOOST:
        raise RuntimeError("xgboost is required for training. Install with: pip install xgboost")

    n = len(X)
    split = int(n * 0.8)
    X_train, X_val = X[:split], X[split:]
    y_train, y_val = y[:split], y[split:]

    # Use GPU if available
    try:
        import torch
        xgb_device = "cuda" if torch.cuda.is_available() else "cpu"
    except ImportError:
        xgb_device = "cpu"

    # XGBoost multi-output: train 4 separate models (one per weight)
    # and combine predictions, then softmax-normalize
    models = []
    for i in range(4):
        dtrain = xgb.DMatrix(X_train, label=y_train[:, i], feature_names=FEATURE_NAMES)
        dval = xgb.DMatrix(X_val, label=y_val[:, i], feature_names=FEATURE_NAMES)

        params = {
            "objective": "reg:squarederror",
            "eval_metric": "mae",
            "max_depth": 4,
            "learning_rate": 0.05,
            "subsample": 0.8,
            "colsample_bytree": 0.8,
            "device": xgb_device,
            "tree_method": "gpu_hist" if xgb_device == "cuda" else "hist",
            "seed": 42,
        }
        model = xgb.train(
            params, dtrain,
            num_boost_round=150,
            evals=[(dval, "val")],
            early_stopping_rounds=15,
            verbose_eval=False,
        )
        models.append(model)

    # Evaluate: predict all 4 weights, softmax normalize, compute MAE
    dval_all = xgb.DMatrix(X_val, feature_names=FEATURE_NAMES)
    preds = np.column_stack([m.predict(dval_all) for m in models])

    # Softmax normalize each row
    exp_preds = np.exp(preds - preds.max(axis=1, keepdims=True))
    normalized = exp_preds / exp_preds.sum(axis=1, keepdims=True)

    metrics = {}
    if HAS_SKLEARN and len(y_val) > 0:
        metrics["mae"] = float(mean_absolute_error(y_val.ravel(), normalized.ravel()))
    elif len(y_val) > 0:
        metrics["mae"] = float(np.mean(np.abs(y_val - normalized)))

    # Save as a single combined model by saving all 4 as a list
    # The WeightOptimizer class expects a single Booster, but we need 4.
    # Solution: save individually and load from a manifest.
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    for i, model in enumerate(models):
        model.save_model(str(output_path / f"weight_optimizer_{i}.json"))

    # Save manifest
    manifest = {"n_models": 4, "feature_names": FEATURE_NAMES}
    with open(output_path / "weight_optimizer_manifest.json", "w") as f:
        json.dump(manifest, f)

    logger.info("Weight optimizer metrics: %s", metrics)
    return metrics


def run_training(db_path: str, output_dir: str, min_samples: int = 100, force: bool = False):
    """Run full training pipeline.

    Args:
        db_path: Path to portfolio.db
        output_dir: Directory to save models
        min_samples: Skip training if fewer samples available
        force: Force retrain even if insufficient samples
    """
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
    start = time.time()

    logger.info("=" * 60)
    logger.info("Signal Models Training Pipeline")
    logger.info("=" * 60)
    logger.info("DB: %s", db_path)
    logger.info("Output: %s", output_dir)

    # --- Meta-Model ---
    logger.info("\n--- Training Meta-Model ---")
    X_meta, y_cls, y_reg = prepare_meta_model_data(db_path)
    if len(X_meta) < min_samples and not force:
        logger.warning("Insufficient data for meta-model: %d samples (need %d). Skipping.",
                       len(X_meta), min_samples)
    elif len(X_meta) > 0:
        logger.info("Training meta-model with %d samples", len(X_meta))
        meta_metrics = train_meta_model(X_meta, y_cls, y_reg, output_dir)
        logger.info("Meta-model metrics: %s", meta_metrics)
    else:
        logger.warning("No training data found for meta-model.")

    # --- Calibrators ---
    logger.info("\n--- Training Confidence Calibrators ---")
    cal_data = prepare_calibration_data(db_path)
    if cal_data:
        cal_metrics = train_calibrators(cal_data, output_dir)
        logger.info("Calibrator metrics: %s", cal_metrics)
    else:
        logger.warning("No calibration data available. Skipping.")

    # --- Weight Optimizer ---
    logger.info("\n--- Training Weight Optimizer ---")
    X_weights, y_weights = prepare_weight_data(db_path)
    if len(X_weights) < min_samples and not force:
        logger.warning("Insufficient data for weight optimizer: %d samples (need %d). Skipping.",
                       len(X_weights), min_samples)
    elif len(X_weights) > 0:
        logger.info("Training weight optimizer with %d samples", len(X_weights))
        weight_metrics = train_weight_optimizer(X_weights, y_weights, output_dir)
        logger.info("Weight optimizer metrics: %s", weight_metrics)
    else:
        logger.warning("No training data found for weight optimizer.")

    elapsed = time.time() - start
    logger.info("\n" + "=" * 60)
    logger.info("Training complete in %.1fs", elapsed)
    logger.info("=" * 60)


def main():
    parser = argparse.ArgumentParser(description="Train signal models")
    parser.add_argument("--db-path", default="../portfolio.db", help="Path to portfolio.db")
    parser.add_argument("--output-dir", default="./models/signal_models", help="Model output directory")
    parser.add_argument("--min-samples", type=int, default=100, help="Min samples to train")
    parser.add_argument("--retrain", action="store_true", help="Force retrain")
    args = parser.parse_args()

    run_training(args.db_path, args.output_dir, args.min_samples, args.retrain)


if __name__ == "__main__":
    main()
