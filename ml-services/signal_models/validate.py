"""Model validation gates for Signal Models.

Run after training to verify models meet minimum quality thresholds.
Usage: python -m signal_models.validate [--model-dir ./models/signal_models]
"""
import sys
import json
import logging
import argparse
import numpy as np
from pathlib import Path

logger = logging.getLogger(__name__)

# Minimum thresholds for model promotion
THRESHOLDS = {
    "meta_model_accuracy": 0.52,
    "meta_model_auc": 0.55,
    "calibrator_max_error": 0.15,
    "weight_sum_tolerance": 0.05,  # weights must sum to 1.0 +/- this
    "max_single_weight": 0.50,
}


def validate_meta_model(model_dir: str) -> dict:
    """Validate the meta model meets minimum accuracy and AUC thresholds."""
    result = {"passed": False, "checks": []}

    metrics_path = Path(model_dir) / "meta_model_metrics.json"
    if not metrics_path.exists():
        result["checks"].append({"name": "metrics_file", "passed": False, "reason": "meta_model_metrics.json not found"})
        return result

    with open(metrics_path) as f:
        metrics = json.load(f)

    accuracy = metrics.get("accuracy", 0)
    auc = metrics.get("auc", 0)

    acc_ok = accuracy >= THRESHOLDS["meta_model_accuracy"]
    result["checks"].append({
        "name": "accuracy",
        "passed": acc_ok,
        "value": accuracy,
        "threshold": THRESHOLDS["meta_model_accuracy"],
    })

    auc_ok = auc >= THRESHOLDS["meta_model_auc"]
    result["checks"].append({
        "name": "auc",
        "passed": auc_ok,
        "value": auc,
        "threshold": THRESHOLDS["meta_model_auc"],
    })

    result["passed"] = acc_ok and auc_ok
    return result


def validate_calibrator(model_dir: str) -> dict:
    """Validate the confidence calibrator has acceptable calibration error."""
    result = {"passed": False, "checks": []}

    metrics_path = Path(model_dir) / "calibrator_metrics.json"
    if not metrics_path.exists():
        result["checks"].append({"name": "metrics_file", "passed": False, "reason": "calibrator_metrics.json not found"})
        # Pass if no calibrator (optional model)
        result["passed"] = True
        result["checks"][-1]["reason"] += " (optional, passing)"
        return result

    with open(metrics_path) as f:
        metrics = json.load(f)

    max_error = max(metrics.get("per_engine_error", {}).values(), default=0)

    error_ok = max_error <= THRESHOLDS["calibrator_max_error"]
    result["checks"].append({
        "name": "calibration_error",
        "passed": error_ok,
        "value": max_error,
        "threshold": THRESHOLDS["calibrator_max_error"],
    })

    result["passed"] = error_ok
    return result


def validate_weights(model_dir: str) -> dict:
    """Validate the weight optimizer produces reasonable weights."""
    result = {"passed": False, "checks": []}

    manifest_path = Path(model_dir) / "weight_optimizer_manifest.json"
    single_path = Path(model_dir) / "weight_optimizer.json"

    if not manifest_path.exists() and not single_path.exists():
        result["checks"].append({"name": "model_file", "passed": False, "reason": "weight optimizer not found"})
        result["passed"] = True  # Optional model
        result["checks"][-1]["reason"] += " (optional, passing)"
        return result

    # Test with default feature vector
    try:
        from .features import FEATURE_NAMES
        test_features = np.zeros(len(FEATURE_NAMES))
        # Set reasonable defaults
        test_features[0:4] = [0, 0, 0, 0]  # signals
        test_features[4:8] = [0.5, 0.5, 0.5, 0.5]  # confidences

        from .model import WeightOptimizer
        wo = WeightOptimizer(model_dir)
        if wo.load():
            weights = wo.predict_weights(test_features)
        else:
            # Try multi-model format
            import xgboost as xgb
            with open(manifest_path) as f:
                manifest = json.load(f)
            n_models = manifest.get("n_models", 4)
            models = []
            for i in range(n_models):
                m = xgb.Booster()
                m.load_model(str(Path(model_dir) / f"weight_optimizer_{i}.json"))
                models.append(m)

            dmat = xgb.DMatrix(test_features.reshape(1, -1), feature_names=FEATURE_NAMES)
            raw = np.array([m.predict(dmat)[0] for m in models])
            exp_vals = np.exp(raw - np.max(raw))
            w = exp_vals / exp_vals.sum()
            weights = {
                "technical": float(w[0]),
                "fundamental": float(w[1]),
                "quantitative": float(w[2]),
                "sentiment": float(w[3]),
            }

        weight_sum = sum(weights.values())
        sum_ok = abs(weight_sum - 1.0) <= THRESHOLDS["weight_sum_tolerance"]
        result["checks"].append({
            "name": "weight_sum",
            "passed": sum_ok,
            "value": weight_sum,
            "threshold": f"1.0 +/- {THRESHOLDS['weight_sum_tolerance']}",
        })

        max_w = max(weights.values())
        max_ok = max_w <= THRESHOLDS["max_single_weight"]
        result["checks"].append({
            "name": "max_single_weight",
            "passed": max_ok,
            "value": max_w,
            "threshold": THRESHOLDS["max_single_weight"],
        })

        result["passed"] = sum_ok and max_ok

    except Exception as e:
        result["checks"].append({"name": "prediction_test", "passed": False, "reason": str(e)})

    return result


def run_all_validations(model_dir: str) -> bool:
    """Run all model validations. Returns True if all pass."""
    logger.info("Running model validation gates on %s", model_dir)

    results = {
        "meta_model": validate_meta_model(model_dir),
        "calibrator": validate_calibrator(model_dir),
        "weights": validate_weights(model_dir),
    }

    all_passed = True
    for name, result in results.items():
        status = "PASS" if result["passed"] else "FAIL"
        logger.info("[%s] %s", status, name)
        for check in result["checks"]:
            check_status = "OK" if check["passed"] else "FAIL"
            detail = f"value={check.get('value', 'N/A')}, threshold={check.get('threshold', 'N/A')}"
            if "reason" in check:
                detail = check["reason"]
            logger.info("  [%s] %s: %s", check_status, check["name"], detail)
        if not result["passed"]:
            all_passed = False

    return all_passed


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(message)s")

    parser = argparse.ArgumentParser(description="Validate trained signal models")
    parser.add_argument("--model-dir", default="./models/signal_models", help="Path to model directory")
    args = parser.parse_args()

    passed = run_all_validations(args.model_dir)
    sys.exit(0 if passed else 1)
