"""Feature drift detection using Population Stability Index (PSI).

Compares recent analysis features against training distribution statistics.
PSI < 0.1: no significant drift
PSI 0.1-0.25: moderate drift (monitor)
PSI > 0.25: significant drift (retrain recommended)
"""
import json
import logging
import argparse
import numpy as np
from pathlib import Path
from typing import Dict, List, Optional, Tuple

logger = logging.getLogger(__name__)

PSI_THRESHOLD_MODERATE = 0.10
PSI_THRESHOLD_SIGNIFICANT = 0.25
N_BINS = 10


def compute_psi(expected: np.ndarray, actual: np.ndarray, n_bins: int = N_BINS) -> float:
    """Compute Population Stability Index between two distributions.

    Args:
        expected: reference distribution (training data)
        actual: current distribution (recent data)
        n_bins: number of bins for histogram

    Returns:
        PSI value (0 = identical distributions)
    """
    # Create bins from expected distribution
    breakpoints = np.linspace(
        min(expected.min(), actual.min()) - 1e-6,
        max(expected.max(), actual.max()) + 1e-6,
        n_bins + 1,
    )

    expected_counts = np.histogram(expected, bins=breakpoints)[0]
    actual_counts = np.histogram(actual, bins=breakpoints)[0]

    # Convert to proportions (add small epsilon to avoid division by zero)
    eps = 1e-4
    expected_pct = (expected_counts + eps) / (expected_counts.sum() + eps * n_bins)
    actual_pct = (actual_counts + eps) / (actual_counts.sum() + eps * n_bins)

    psi = np.sum((actual_pct - expected_pct) * np.log(actual_pct / expected_pct))
    return float(psi)


def load_training_stats(model_dir: str) -> Optional[Dict]:
    """Load training feature statistics (mean, std, quantiles per feature)."""
    stats_path = Path(model_dir) / "training_feature_stats.json"
    if not stats_path.exists():
        return None
    with open(stats_path) as f:
        return json.load(f)


def save_training_stats(features: np.ndarray, feature_names: List[str], model_dir: str):
    """Save training feature statistics for future drift comparison."""
    stats = {}
    for i, name in enumerate(feature_names):
        col = features[:, i]
        stats[name] = {
            "mean": float(np.mean(col)),
            "std": float(np.std(col)),
            "min": float(np.min(col)),
            "max": float(np.max(col)),
            "q25": float(np.percentile(col, 25)),
            "q50": float(np.percentile(col, 50)),
            "q75": float(np.percentile(col, 75)),
            "values": col.tolist(),  # Store full distribution for PSI
        }

    stats_path = Path(model_dir) / "training_feature_stats.json"
    with open(stats_path, "w") as f:
        json.dump(stats, f, indent=2)
    logger.info("Saved training stats for %d features to %s", len(feature_names), stats_path)


def check_feature_drift(
    recent_features: np.ndarray,
    feature_names: List[str],
    model_dir: str,
) -> Dict[str, Dict]:
    """Check for feature drift between training and recent data.

    Args:
        recent_features: array of shape (n_samples, n_features) from recent analysis
        feature_names: list of feature names
        model_dir: path to model directory with training_feature_stats.json

    Returns:
        Dict mapping feature name to {psi, status, mean_shift}
    """
    training_stats = load_training_stats(model_dir)
    if training_stats is None:
        logger.warning("No training stats found at %s — cannot check drift", model_dir)
        return {}

    results = {}
    for i, name in enumerate(feature_names):
        if name not in training_stats:
            continue

        train_info = training_stats[name]
        train_values = np.array(train_info.get("values", []))

        if len(train_values) < N_BINS or recent_features.shape[0] < N_BINS:
            results[name] = {"psi": 0.0, "status": "insufficient_data", "mean_shift": 0.0}
            continue

        recent_col = recent_features[:, i]
        psi = compute_psi(train_values, recent_col)

        mean_shift = abs(float(np.mean(recent_col)) - train_info["mean"])

        if psi > PSI_THRESHOLD_SIGNIFICANT:
            status = "significant_drift"
        elif psi > PSI_THRESHOLD_MODERATE:
            status = "moderate_drift"
        else:
            status = "stable"

        results[name] = {
            "psi": round(psi, 4),
            "status": status,
            "mean_shift": round(mean_shift, 4),
        }

    return results


def summarize_drift(drift_results: Dict[str, Dict]) -> Tuple[str, int, int]:
    """Summarize drift results into an overall status.

    Returns:
        (overall_status, n_significant, n_moderate)
    """
    n_significant = sum(1 for r in drift_results.values() if r["status"] == "significant_drift")
    n_moderate = sum(1 for r in drift_results.values() if r["status"] == "moderate_drift")
    n_total = len(drift_results)

    if n_significant > n_total * 0.3:
        return "retrain_recommended", n_significant, n_moderate
    elif n_significant > 0 or n_moderate > n_total * 0.5:
        return "monitor", n_significant, n_moderate
    else:
        return "stable", n_significant, n_moderate


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(message)s")

    parser = argparse.ArgumentParser(description="Check feature drift for signal models")
    parser.add_argument("--model-dir", default="./models/signal_models")
    parser.add_argument("--db-url", default="sqlite:../../portfolio.db", help="Database URL for analysis_features table")
    parser.add_argument("--days", type=int, default=7, help="Look back N days for recent features")
    args = parser.parse_args()

    # Try to load recent features from DB
    try:
        import sqlite3
        db_path = args.db_url.replace("sqlite:", "")
        conn = sqlite3.connect(db_path)
        cursor = conn.execute(
            "SELECT features_json FROM analysis_features WHERE created_at > datetime('now', ?) ORDER BY created_at DESC LIMIT 500",
            (f"-{args.days} days",),
        )
        rows = cursor.fetchall()
        conn.close()

        if not rows:
            logger.info("No recent features found in database (last %d days)", args.days)
            exit(0)

        from .features import FEATURE_NAMES, features_to_array
        feature_arrays = []
        for (features_json,) in rows:
            features_dict = json.loads(features_json)
            feature_arrays.append(features_to_array(features_dict))

        recent = np.array(feature_arrays)
        results = check_feature_drift(recent, FEATURE_NAMES, args.model_dir)

        overall, n_sig, n_mod = summarize_drift(results)
        logger.info("Drift summary: %s (%d significant, %d moderate out of %d features)",
                     overall, n_sig, n_mod, len(results))

        for name, info in sorted(results.items(), key=lambda x: -x[1]["psi"]):
            if info["status"] != "stable":
                logger.info("  [%s] %s: PSI=%.4f, mean_shift=%.4f",
                           info["status"].upper(), name, info["psi"], info["mean_shift"])

        exit(0 if overall == "stable" else 1)

    except Exception as e:
        logger.error("Failed to check drift: %s", e)
        exit(1)
