#!/usr/bin/env bash
set -euo pipefail

# Retrain signal models with validation gates.
# Usage: ./scripts/retrain-with-validation.sh [--force]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ML_DIR="$PROJECT_ROOT/ml-services"
MODEL_DIR="${MODEL_DIR:-$ML_DIR/models/signal_models}"
TEMP_DIR="$MODEL_DIR/staging"
FORCE="${1:-}"

echo "=== InvestIQ Signal Models Retraining ==="
echo "Model directory: $MODEL_DIR"
echo "Temp directory: $TEMP_DIR"

# Step 1: Check drift (skip if --force)
if [ "$FORCE" != "--force" ]; then
    echo ""
    echo "--- Step 1: Checking feature drift ---"
    cd "$ML_DIR"
    if python -m signal_models.drift --model-dir "$MODEL_DIR" 2>/dev/null; then
        echo "No significant drift detected. Skipping retraining."
        echo "Use --force to retrain anyway."
        exit 0
    fi
    echo "Drift detected — proceeding with retraining."
fi

# Step 2: Train to temp directory
echo ""
echo "--- Step 2: Training models to staging directory ---"
mkdir -p "$TEMP_DIR"
cd "$ML_DIR"
MODEL_DIR="$TEMP_DIR" python -m signal_models.train

# Step 3: Validate new models
echo ""
echo "--- Step 3: Validating trained models ---"
if python -m signal_models.validate --model-dir "$TEMP_DIR"; then
    echo "Validation PASSED."
else
    echo "Validation FAILED. New models rejected."
    rm -rf "$TEMP_DIR"
    exit 1
fi

# Step 4: Promote models
echo ""
echo "--- Step 4: Promoting validated models ---"
# Backup current models
if [ -d "$MODEL_DIR" ] && [ "$(ls -A "$MODEL_DIR" 2>/dev/null)" ]; then
    BACKUP_DIR="$MODEL_DIR/backup_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$BACKUP_DIR"
    find "$MODEL_DIR" -maxdepth 1 -type f -exec cp {} "$BACKUP_DIR/" \;
    echo "Backed up current models to $BACKUP_DIR"
fi

# Move staged models to production
find "$TEMP_DIR" -maxdepth 1 -type f -exec mv {} "$MODEL_DIR/" \;
rmdir "$TEMP_DIR" 2>/dev/null || true

echo ""
echo "=== Retraining complete. Models promoted. ==="
