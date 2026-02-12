#!/usr/bin/env bash
# Launch InvestIQ as a native desktop application (single binary).
#
# The desktop app embeds:
#   - Rust API server (port 3000)
#   - Python Dash frontend files (extracted to ~/.investiq/frontend/ on first run)
#
# Prerequisites:
#   - Python 3.8+ in PATH (for the Dash frontend)
#   - .env file configured (copy from .env.example)
#
# Usage:
#   ./start-desktop.sh          # Build and run (debug mode)
#   ./start-desktop.sh --release # Build and run (release mode)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CARGO_FLAGS=""
if [[ "$1" == "--release" ]]; then
    CARGO_FLAGS="--release"
fi

echo "=== InvestIQ Desktop ==="
echo "Building and launching..."
cargo run --package desktop-app $CARGO_FLAGS
