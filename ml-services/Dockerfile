FROM python:3.11-slim

WORKDIR /app

# Install system dependencies
RUN apt-get update && \
    apt-get install -y curl && \
    rm -rf /var/lib/apt/lists/*

# Copy and install only the signal_models dependencies (lighter than full requirements.txt)
COPY signal_models/requirements.txt ./requirements-signal.txt
RUN pip install --no-cache-dir -r requirements-signal.txt 2>/dev/null || \
    pip install --no-cache-dir fastapi uvicorn[standard] pydantic scikit-learn xgboost numpy scipy

# Copy application
COPY signal_models ./signal_models

# Create models directory
RUN mkdir -p /app/models

EXPOSE 8004

ENV PYTHONUNBUFFERED=1

HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8004/health || exit 1

CMD ["python", "-m", "uvicorn", "signal_models.service:app", "--host", "0.0.0.0", "--port", "8004"]
