use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};

/// Adds OWASP-recommended security headers to every response.
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    // Modern approach: CSP replaces XSS protection; setting to 0 disables legacy filter
    headers.insert("x-xss-protection", HeaderValue::from_static("0"));
    // API-only CSP: no resources should be loaded
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    // Financial data must not be cached
    headers.insert("cache-control", HeaderValue::from_static("no-store"));

    // HSTS only when explicitly enabled (requires TLS termination)
    let enable_hsts = std::env::var("ENABLE_HSTS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    if enable_hsts {
        headers.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
        );
    }

    response
}
