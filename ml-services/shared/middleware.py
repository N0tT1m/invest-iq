"""Shared production hardening middleware for all ML services.

Call `setup_hardening(app, service_name)` once after creating the FastAPI app.
Adds: request timeout, error sanitization, request size limit, structured JSON
logging, request-ID propagation, and a Prometheus /metrics endpoint.
"""
import asyncio
import logging
import os
import time
import uuid

from fastapi import Request
from fastapi.responses import JSONResponse
from starlette.middleware.base import BaseHTTPMiddleware

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Defaults (overridable via env vars)
# ---------------------------------------------------------------------------
DEFAULT_REQUEST_TIMEOUT = 30  # seconds
MAX_BODY_SIZE = 10 * 1024 * 1024  # 10 MB


# ---------------------------------------------------------------------------
# Prometheus metrics (lazy-init so import never fails)
# ---------------------------------------------------------------------------
def _init_prometheus(service_name: str):
    """Initialise Prometheus collectors. Returns (counter, histogram, gauge)."""
    try:
        from prometheus_client import Counter, Gauge, Histogram

        request_count = Counter(
            "http_requests_total",
            "Total HTTP requests",
            ["service", "method", "path", "status"],
        )
        request_latency = Histogram(
            "http_request_duration_seconds",
            "Request latency in seconds",
            ["service", "method", "path"],
            buckets=(0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30),
        )
        in_flight = Gauge(
            "http_requests_in_flight",
            "Requests currently being processed",
            ["service"],
        )
        return request_count, request_latency, in_flight
    except ImportError:
        logger.warning("prometheus_client not installed -- /metrics will be unavailable")
        return None, None, None


# ---------------------------------------------------------------------------
# setup_hardening()
# ---------------------------------------------------------------------------
def setup_hardening(app, service_name: str) -> None:
    """Register all production middleware + /metrics on *app*."""

    timeout_sec = int(os.getenv("ML_REQUEST_TIMEOUT", str(DEFAULT_REQUEST_TIMEOUT)))
    max_body = int(os.getenv("ML_MAX_BODY_SIZE", str(MAX_BODY_SIZE)))

    # ---- Structured JSON logging ----
    if os.getenv("LOG_FORMAT", "").lower() == "json":
        _configure_json_logging(service_name)

    # ---- Prometheus ----
    req_count, req_latency, req_inflight = _init_prometheus(service_name)

    # ---- Metrics endpoint ----
    try:
        from prometheus_client import generate_latest, CONTENT_TYPE_LATEST

        @app.get("/metrics", include_in_schema=False)
        async def metrics():
            return JSONResponse(
                content=generate_latest().decode(),
                media_type=CONTENT_TYPE_LATEST,
            )
    except ImportError:
        pass

    # ---- Combined middleware (single layer to minimise overhead) ----
    class HardeningMiddleware(BaseHTTPMiddleware):
        async def dispatch(self, request: Request, call_next):
            # -- Request ID --
            request_id = request.headers.get("x-request-id") or str(uuid.uuid4())

            # -- Size limit (skip /health and /metrics) --
            path = request.url.path
            if path not in ("/health", "/metrics"):
                cl = request.headers.get("content-length")
                if cl and int(cl) > max_body:
                    return JSONResponse(
                        status_code=413,
                        content={"error": "Payload too large"},
                        headers={"X-Request-ID": request_id},
                    )

            # -- Prometheus in-flight gauge --
            if req_inflight is not None:
                req_inflight.labels(service=service_name).inc()

            start = time.monotonic()
            status_code = 500
            try:
                response = await asyncio.wait_for(
                    call_next(request), timeout=timeout_sec
                )
                status_code = response.status_code
                response.headers["X-Request-ID"] = request_id
                return response
            except asyncio.TimeoutError:
                status_code = 504
                logger.error(
                    "Request timeout after %ds: %s %s",
                    timeout_sec,
                    request.method,
                    path,
                )
                return JSONResponse(
                    status_code=504,
                    content={"error": "Request timeout"},
                    headers={"X-Request-ID": request_id},
                )
            except Exception:
                logger.exception("Unhandled error: %s %s", request.method, path)
                return JSONResponse(
                    status_code=500,
                    content={"error": "Internal server error"},
                    headers={"X-Request-ID": request_id},
                )
            finally:
                elapsed = time.monotonic() - start
                if req_inflight is not None:
                    req_inflight.labels(service=service_name).dec()
                if req_count is not None:
                    req_count.labels(
                        service=service_name,
                        method=request.method,
                        path=path,
                        status=str(status_code),
                    ).inc()
                if req_latency is not None:
                    req_latency.labels(
                        service=service_name,
                        method=request.method,
                        path=path,
                    ).observe(elapsed)

    app.add_middleware(HardeningMiddleware)


# ---------------------------------------------------------------------------
# JSON logging helper
# ---------------------------------------------------------------------------
def _configure_json_logging(service_name: str) -> None:
    """Switch root logger to structured JSON output."""
    try:
        from pythonjsonlogger import jsonlogger

        handler = logging.StreamHandler()
        formatter = jsonlogger.JsonFormatter(
            fmt="%(asctime)s %(levelname)s %(name)s %(message)s",
            rename_fields={"asctime": "timestamp", "levelname": "level"},
        )
        handler.setFormatter(formatter)

        root = logging.getLogger()
        root.handlers.clear()
        root.addHandler(handler)
        root.setLevel(logging.INFO)
        logging.getLogger(service_name).info("JSON logging enabled for %s", service_name)
    except ImportError:
        logger.warning(
            "python-json-logger not installed -- falling back to plain text logging"
        )
