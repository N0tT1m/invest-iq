#!/usr/bin/env bash
# Upload ML models to a GitHub release.
# Usage: ./scripts/upload-models.sh v1.0.1
#
# Packages signal_models + price_predictor into ml-models.zip (~10 MB)
# and uploads to the specified release.
# FinBERT (836 MB) is excluded — it auto-downloads on first run.

set -euo pipefail

TAG="${1:?Usage: $0 <tag> (e.g. v1.0.1)}"
REPO="N0tT1m/invest-iq"
MODELS_DIR="ml-services/models"
ZIP_FILE="ml-models.zip"

# Check models exist
if [ ! -d "$MODELS_DIR/signal_models" ] && [ ! -d "$MODELS_DIR/price_predictor" ]; then
    echo "No models found in $MODELS_DIR. Train models first."
    exit 1
fi

echo "Packaging models (excluding FinBERT)..."
zip -r "$ZIP_FILE" \
    "$MODELS_DIR/signal_models/" \
    "$MODELS_DIR/price_predictor/" \
    -x "*.pyc" "*__pycache__*"

SIZE=$(du -h "$ZIP_FILE" | cut -f1)
echo "Created $ZIP_FILE ($SIZE)"

echo "Uploading to release $TAG..."
gh release upload "$TAG" "$ZIP_FILE" --repo "$REPO" --clobber

rm "$ZIP_FILE"
echo "Done! Models attached to $TAG"
