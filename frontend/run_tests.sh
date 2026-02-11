#!/bin/bash
# Test runner script for InvestIQ frontend tests

set -e  # Exit on error

echo "=========================================="
echo "InvestIQ Frontend Test Suite"
echo "=========================================="
echo ""

# Check if pytest is installed
if ! python -m pytest --version > /dev/null 2>&1; then
    echo "❌ pytest not found. Installing test dependencies..."
    pip install -r requirements.txt
    echo ""
fi

# Parse command line arguments
PYTEST_ARGS="$@"

# Default arguments if none provided
if [ -z "$PYTEST_ARGS" ]; then
    PYTEST_ARGS="-v --tb=short"
fi

echo "Running tests with: pytest $PYTEST_ARGS"
echo ""

# Run pytest
python -m pytest $PYTEST_ARGS

# Check exit code
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ All tests passed!"
else
    echo ""
    echo "❌ Some tests failed. See output above for details."
    exit 1
fi
