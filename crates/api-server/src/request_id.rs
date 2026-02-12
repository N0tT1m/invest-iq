use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Extension type to carry the request ID through handlers.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RequestId(pub String);

/// Generates or propagates a UUID v4 request ID for every request.
///
/// - Checks for incoming `X-Request-Id` header (from reverse proxy) and reuses it
/// - Otherwise generates a new UUID v4
/// - Inserts into request extensions as `RequestId(String)` for handler access
/// - Adds `X-Request-Id` header to the response
pub async fn request_id_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    tracing::Span::current().record("request_id", &id.as_str());

    request.extensions_mut().insert(RequestId(id.clone()));

    let mut response = next.run(request).await;
    if let Ok(val) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}
