#[cfg(test)]
mod tests {
    use super::super::*;
    use axum::body::Body;
    use axum::http::{HeaderMap, HeaderValue, Request};

    #[test]
    fn test_mask_api_key() {
        let key = "abcd1234efgh5678";
        let masked = mask_api_key(&key);
        assert_eq!(masked, "abcd...5678");
    }

    #[test]
    fn test_mask_short_api_key() {
        let key = "short";
        let masked = mask_api_key(&key);
        assert_eq!(masked, "****");
    }

    #[test]
    fn test_extract_api_key_from_x_api_key_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("test_key_123"));

        let request = Request::builder()
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let result = extract_api_key(&headers, &request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_key_123");
    }

    #[test]
    fn test_extract_api_key_from_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer test_token_456"),
        );

        let request = Request::builder()
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let result = extract_api_key(&headers, &request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_token_456");
    }

    #[test]
    fn test_extract_api_key_from_query_param() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .uri("/api/test?api_key=query_key_789")
            .body(Body::empty())
            .unwrap();

        let result = extract_api_key(&headers, &request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "query_key_789");
    }

    #[test]
    fn test_extract_api_key_missing() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        let result = extract_api_key(&headers, &request);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::MissingApiKey));
    }

    #[test]
    fn test_extract_api_key_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("x_api_key"));
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer bearer_token"),
        );

        let request = Request::builder()
            .uri("/api/test?api_key=query_key")
            .body(Body::empty())
            .unwrap();

        let result = extract_api_key(&headers, &request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "x_api_key");
    }

    #[test]
    fn test_get_valid_api_keys() {
        std::env::set_var("API_KEYS", "key1,key2,key3");

        let keys = get_valid_api_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains("key1"));
        assert!(keys.contains("key2"));
        assert!(keys.contains("key3"));

        std::env::remove_var("API_KEYS");
    }

    #[test]
    fn test_get_valid_api_keys_with_whitespace() {
        std::env::set_var("API_KEYS", " key1 , key2 , key3 ");

        let keys = get_valid_api_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains("key1"));
        assert!(keys.contains("key2"));
        assert!(keys.contains("key3"));

        std::env::remove_var("API_KEYS");
    }

    // Note: env-var-based tests for empty API_KEYS are omitted because
    // they race with parallel tests that also set API_KEYS.

    #[test]
    fn test_validated_api_key_clone() {
        let key1 = ValidatedApiKey("test_key".to_string());
        let key2 = key1.clone();

        assert_eq!(key1.0, key2.0);
    }
}
