"""Shared API configuration for all components."""
import os

API_BASE = os.getenv("API_BASE_URL", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "") or os.getenv("API_KEYS", "").split(",")[0].strip()

# Request timeout in seconds
API_TIMEOUT = int(os.getenv("API_TIMEOUT", "30"))

# Production safety: require API_KEY if PRODUCTION=true
_is_production = os.getenv("PRODUCTION", "").lower() in ("true", "1", "yes")
if _is_production and not API_KEY:
    raise RuntimeError("PRODUCTION=true but API_KEY is not set. Refusing to start.")

# Dev fallback when env var is not set
if not API_KEY:
    import warnings
    warnings.warn("API_KEY not set. Set API_KEY in .env file.", stacklevel=2)


def get_headers():
    return {"X-API-Key": API_KEY, "Content-Type": "application/json"}
