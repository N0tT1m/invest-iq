#!/usr/bin/env bash
# Create a Python venv matching the system Python (for PyO3 compatibility)
# and install the core ML dependencies.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="${1:-$SCRIPT_DIR/venv314}"
PYTHON="${PYTHON:-python3}"

PYTHON_VERSION=$($PYTHON --version 2>&1 | awk '{print $2}' | cut -d. -f1,2)
echo "Using Python $PYTHON_VERSION ($($PYTHON -c 'import sys; print(sys.executable)'))"
echo "Creating venv at: $VENV_DIR"

$PYTHON -m venv "$VENV_DIR"
"$VENV_DIR/bin/pip" install --upgrade pip -q
"$VENV_DIR/bin/pip" install -r "$SCRIPT_DIR/requirements-core.txt"

echo ""
echo "Venv ready at: $VENV_DIR"
echo "Add to .env:  VIRTUAL_ENV=$VENV_DIR"
