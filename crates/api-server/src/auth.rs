use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Hash a key with SHA-256 for constant-time-safe HashMap lookup.
/// By storing and comparing hashes (fixed 64-char hex) instead of raw keys,
/// the HashMap lookup timing does not leak information about the key value.
fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod auth_tests;

/// Role hierarchy for RBAC
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer = 0,
    Trader = 1,
    Admin = 2,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "viewer" => Some(Role::Viewer),
            "trader" => Some(Role::Trader),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Viewer => write!(f, "viewer"),
            Role::Trader => write!(f, "trader"),
            Role::Admin => write!(f, "admin"),
        }
    }
}

/// API key authentication middleware with brute-force protection.
///
/// Checks for API key in:
/// 1. X-API-Key header (recommended)
/// 2. Authorization: Bearer <token> header
///
/// If API_KEYS env var is not set, authentication is skipped (development mode).
/// Uses `from_fn_with_state` so it can access `AppState.brute_force_guard`.
pub async fn auth_middleware(
    State(state): State<crate::AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let valid_keys = get_valid_api_keys();

    // Skip authentication for health check and metrics endpoints
    let path = request.uri().path();
    if path == "/" || path == "/health" || path == "/metrics" || path == "/metrics/json" {
        return Ok(next.run(request).await);
    }

    // If no API keys configured, skip authentication (development mode)
    if valid_keys.is_empty() {
        return Ok(next.run(request).await);
    }

    let ip = connect_info
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check brute-force lockout before attempting validation
    if state.brute_force_guard.is_locked(&ip) {
        return Err(AuthError::Locked);
    }

    // Try to extract API key from various sources
    let api_key = extract_api_key(&headers, &request)?;

    // Hash the provided key and look up in the hashed-key HashMap
    let key_hash = hash_key(&api_key);
    let role = match valid_keys.get(&key_hash) {
        Some(role) => {
            state.brute_force_guard.record_success(&ip);
            *role
        }
        None => {
            tracing::warn!("Invalid API key attempted: {}", mask_api_key(&api_key));
            state.brute_force_guard.record_failure(&ip);
            return Err(AuthError::InvalidApiKey);
        }
    };

    tracing::debug!("Valid API key: {} (role: {})", mask_api_key(&api_key), role);

    // Add the validated API key and role to request extensions
    request
        .extensions_mut()
        .insert(ValidatedApiKey { key: api_key, role });

    Ok(next.run(request).await)
}

/// Extract API key from request headers or query parameters
pub(crate) fn extract_api_key(
    headers: &HeaderMap,
    _request: &Request,
) -> Result<String, AuthError> {
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

    Err(AuthError::MissingApiKey)
}

/// Get valid API keys from environment with roles
///
/// Supports multiple API keys separated by commas with optional role suffix:
/// API_KEYS=key1:admin,key2:trader,key3:viewer,key4
///
/// If no role is specified, defaults to Admin for backwards compatibility.
pub(crate) fn get_valid_api_keys() -> HashMap<String, Role> {
    std::env::var("API_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }

            // Parse "key:role" format, default to Admin if no role specified.
            // Keys are hashed with SHA-256 before storing so lookups compare
            // fixed-length hashes, eliminating timing side-channels.
            if let Some((key, role_str)) = entry.split_once(':') {
                let key = key.trim();
                let role = Role::from_str(role_str.trim()).unwrap_or(Role::Admin);
                Some((hash_key(key), role))
            } else {
                // No role specified, default to Admin (backwards compatible)
                Some((hash_key(entry), Role::Admin))
            }
        })
        .collect()
}

/// Mask API key for logging (show first 4 and last 4 characters)
pub(crate) fn mask_api_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &key[..4], &key[key.len() - 4..])
}

/// Extension type to store validated API key and role in request
#[derive(Clone, Debug)]
pub struct ValidatedApiKey {
    #[allow(dead_code)]
    pub key: String,
    pub role: Role,
}

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
            tracing::warn!(
                "Live trading write endpoint called but LIVE_TRADING_KEY not configured"
            );
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

            // Compare hashes instead of raw strings to prevent timing attacks
            if hash_key(provided) != hash_key(&expected) {
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
    InsufficientRole(Role),
    Locked,
    LiveTradingNotConfigured,
    MissingLiveTradingKey,
    InvalidLiveTradingKey,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingApiKey => write!(f, "Missing API key"),
            AuthError::InvalidApiKey => write!(f, "Invalid API key"),
            AuthError::InsufficientRole(role) => {
                write!(f, "Insufficient permissions. Required role: {}", role)
            }
            AuthError::Locked => write!(f, "Too many failed authentication attempts"),
            AuthError::LiveTradingNotConfigured => write!(f, "Live trading key not configured"),
            AuthError::MissingLiveTradingKey => write!(f, "Missing live trading key"),
            AuthError::InvalidLiveTradingKey => write!(f, "Invalid live trading key"),
        }
    }
}

impl std::error::Error for AuthError {}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingApiKey => (
                StatusCode::UNAUTHORIZED,
                "Missing API key. Provide via X-API-Key header, Authorization: Bearer header, or api_key query parameter.".to_string(),
            ),
            AuthError::InvalidApiKey => (
                StatusCode::FORBIDDEN,
                "Invalid API key.".to_string(),
            ),
            AuthError::Locked => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many failed authentication attempts. Please try again later.".to_string(),
            ),
            AuthError::InsufficientRole(required_role) => (
                StatusCode::FORBIDDEN,
                format!("Insufficient permissions. Required role: {}", required_role),
            ),
            AuthError::LiveTradingNotConfigured => (
                StatusCode::FORBIDDEN,
                "Live trading key not configured. Set LIVE_TRADING_KEY env var to enable broker write endpoints.".to_string(),
            ),
            AuthError::MissingLiveTradingKey => (
                StatusCode::UNAUTHORIZED,
                "Missing live trading key. Provide via X-Live-Trading-Key header.".to_string(),
            ),
            AuthError::InvalidLiveTradingKey => (
                StatusCode::FORBIDDEN,
                "Invalid live trading key.".to_string(),
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

/// Check if request has sufficient role
fn check_role(required: Role, request: &Request) -> Result<(), AuthError> {
    match request.extensions().get::<ValidatedApiKey>() {
        Some(key) if key.role >= required => Ok(()),
        Some(_) => Err(AuthError::InsufficientRole(required)),
        None => Ok(()), // No auth configured (dev mode) - allow through
    }
}

/// Middleware to require trader role or higher
pub async fn require_trader_middleware(
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    check_role(Role::Trader, &request)?;
    Ok(next.run(request).await)
}

/// Middleware to require admin role
pub async fn require_admin_middleware(request: Request, next: Next) -> Result<Response, AuthError> {
    check_role(Role::Admin, &request)?;
    Ok(next.run(request).await)
}

/// Extract role from request extensions and check it meets minimum
/// Returns Ok(()) in dev mode (no API_KEYS configured).
///
/// This is useful for checking roles within handler functions.
#[allow(dead_code)]
pub fn check_role_from_extensions(
    extensions: &axum::http::Extensions,
    required: Role,
) -> Result<(), AuthError> {
    match extensions.get::<ValidatedApiKey>() {
        Some(key) if key.role >= required => Ok(()),
        Some(_) => Err(AuthError::InsufficientRole(required)),
        None => Ok(()), // Dev mode - allow through
    }
}
