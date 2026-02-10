use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::HashSet;

#[cfg(test)]
#[path = "auth_tests.rs"]
mod auth_tests;

/// API key authentication middleware
///
/// Checks for API key in:
/// 1. X-API-Key header (recommended)
/// 2. Authorization: Bearer <token> header
/// 3. api_key query parameter (discouraged, for backward compatibility)
pub async fn auth_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let valid_keys = get_valid_api_keys();

    // Skip authentication for health check and metrics endpoints
    let path = request.uri().path();
    if path == "/" || path == "/health" || path == "/metrics" {
        return Ok(next.run(request).await);
    }

    // Try to extract API key from various sources
    let api_key = extract_api_key(&headers, &request)?;

    // Validate the API key
    if !valid_keys.contains(api_key.as_str()) {
        tracing::warn!("❌ Invalid API key attempted: {}", mask_api_key(&api_key));
        return Err(AuthError::InvalidApiKey);
    }

    tracing::debug!("✅ Valid API key: {}", mask_api_key(&api_key));

    // Add the validated API key to request extensions for potential future use
    request.extensions_mut().insert(ValidatedApiKey(api_key));

    Ok(next.run(request).await)
}

/// Extract API key from request headers or query parameters
pub(crate) fn extract_api_key(headers: &HeaderMap, request: &Request) -> Result<String, AuthError> {
    // 1. Try X-API-Key header (recommended approach)
    if let Some(api_key) = headers.get("X-API-Key") {
        if let Ok(key) = api_key.to_str() {
            if !key.is_empty() {
                return Ok(key.to_string());
            }
        }
    }

    // 2. Try Authorization: Bearer <token> header
    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if !token.is_empty() {
                    return Ok(token.to_string());
                }
            }
        }
    }

    // 3. Try query parameter (discouraged, but supported for backward compatibility)
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "api_key" && !value.is_empty() {
                    return Ok(value.to_string());
                }
            }
        }
    }

    Err(AuthError::MissingApiKey)
}

/// Get valid API keys from environment
///
/// Supports multiple API keys separated by commas:
/// API_KEYS=key1,key2,key3
pub(crate) fn get_valid_api_keys() -> HashSet<String> {
    std::env::var("API_KEYS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Mask API key for logging (show first 4 and last 4 characters)
pub(crate) fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

/// Extension type to store validated API key in request
#[derive(Clone)]
pub struct ValidatedApiKey(pub String);

/// Live trading auth middleware — extra gate for broker write endpoints.
///
/// Checks for `X-Live-Trading-Key` header against `LIVE_TRADING_KEY` env var.
/// If `LIVE_TRADING_KEY` is not set, ALL write broker endpoints are blocked (safe default).
pub async fn live_trading_auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Paper trading doesn't need the live trading key
    let alpaca_url = std::env::var("ALPACA_BASE_URL").unwrap_or_default();
    if alpaca_url.is_empty() || alpaca_url.contains("paper-api") {
        return Ok(next.run(request).await);
    }

    let expected_key = std::env::var("LIVE_TRADING_KEY").ok();

    match expected_key {
        None => {
            // No live trading key configured — block all write endpoints as safety default
            tracing::warn!("Live trading write endpoint called but LIVE_TRADING_KEY not configured");
            Err(AuthError::LiveTradingNotConfigured)
        }
        Some(expected) => {
            let provided = headers
                .get("X-Live-Trading-Key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if provided.is_empty() {
                return Err(AuthError::MissingLiveTradingKey);
            }

            if provided != expected {
                tracing::warn!("Invalid live trading key attempted");
                return Err(AuthError::InvalidLiveTradingKey);
            }

            Ok(next.run(request).await)
        }
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    MissingApiKey,
    InvalidApiKey,
    LiveTradingNotConfigured,
    MissingLiveTradingKey,
    InvalidLiveTradingKey,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingApiKey => (
                StatusCode::UNAUTHORIZED,
                "Missing API key. Provide via X-API-Key header, Authorization: Bearer header, or api_key query parameter.",
            ),
            AuthError::InvalidApiKey => (
                StatusCode::FORBIDDEN,
                "Invalid API key.",
            ),
            AuthError::LiveTradingNotConfigured => (
                StatusCode::FORBIDDEN,
                "Live trading key not configured. Set LIVE_TRADING_KEY env var to enable broker write endpoints.",
            ),
            AuthError::MissingLiveTradingKey => (
                StatusCode::UNAUTHORIZED,
                "Missing live trading key. Provide via X-Live-Trading-Key header.",
            ),
            AuthError::InvalidLiveTradingKey => (
                StatusCode::FORBIDDEN,
                "Invalid live trading key.",
            ),
        };

        (
            status,
            Json(json!({
                "success": false,
                "error": message,
            })),
        )
            .into_response()
    }
}
