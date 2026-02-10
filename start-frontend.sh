#!/bin/bash

# Load environment variables from .env file
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

# Export API_KEY from API_KEYS (use the first key)
if [ -z "$API_KEY" ]; then
    # Extract first API key from API_KEYS
    export API_KEY=$(echo $API_KEYS | cut -d',' -f1)
    echo "✅ Using API_KEY from .env: ${API_KEY:0:8}..."
fi

# Start the frontend application
cd frontend
source venv/bin/activate
python3 app.py
