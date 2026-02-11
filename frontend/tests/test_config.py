"""Tests for components/config.py module."""
import os
import pytest
import importlib
import sys


class TestConfig:
    """Test suite for config module."""

    def test_get_headers_returns_correct_format(self, set_api_env_vars):
        """Test that get_headers() returns correct API key header format."""
        # Reload config module to pick up env vars
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import get_headers

        headers = get_headers()

        assert "X-API-Key" in headers
        assert "Content-Type" in headers
        assert headers["X-API-Key"] == "test-api-key-12345"
        assert headers["Content-Type"] == "application/json"

    def test_get_headers_with_empty_api_key(self, clear_env_vars):
        """Test get_headers() when API_KEY is empty (dev mode)."""
        # Reload config module to pick up cleared env vars
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import get_headers

        headers = get_headers()

        # Should still return headers structure with empty key
        assert "X-API-Key" in headers
        assert headers["X-API-Key"] == ""

    def test_production_safety_gate_raises_when_no_api_key(self, monkeypatch):
        """Test that PRODUCTION=true with no API_KEY raises RuntimeError."""
        # Clear all API-related env vars
        monkeypatch.delenv("API_KEY", raising=False)
        monkeypatch.delenv("API_KEYS", raising=False)
        monkeypatch.setenv("PRODUCTION", "true")

        # Remove cached module to trigger import-time check
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        # Should raise RuntimeError on import
        with pytest.raises(RuntimeError, match="PRODUCTION=true but API_KEY is not set"):
            import components.config

    def test_production_safety_gate_allows_with_api_key(self, monkeypatch):
        """Test that PRODUCTION=true with valid API_KEY does not raise."""
        monkeypatch.setenv("PRODUCTION", "true")
        monkeypatch.setenv("API_KEY", "valid-production-key")

        # Remove cached module
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        # Should not raise
        import components.config
        assert components.config.API_KEY == "valid-production-key"

    def test_api_base_defaults_to_localhost(self, clear_env_vars):
        """Test that API_BASE defaults to localhost when not set."""
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_BASE

        assert API_BASE == "http://localhost:3000"

    def test_api_base_uses_env_var(self, monkeypatch):
        """Test that API_BASE uses environment variable when set."""
        monkeypatch.delenv("API_KEY", raising=False)
        monkeypatch.delenv("API_KEYS", raising=False)
        monkeypatch.setenv("API_BASE_URL", "https://api.example.com")

        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_BASE

        assert API_BASE == "https://api.example.com"

    def test_api_timeout_defaults_to_30(self, clear_env_vars):
        """Test that API_TIMEOUT defaults to 30 seconds."""
        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_TIMEOUT

        assert API_TIMEOUT == 30

    def test_api_timeout_uses_env_var(self, monkeypatch):
        """Test that API_TIMEOUT uses environment variable when set."""
        monkeypatch.delenv("API_KEY", raising=False)
        monkeypatch.delenv("API_KEYS", raising=False)
        monkeypatch.setenv("API_TIMEOUT", "90")

        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_TIMEOUT

        assert API_TIMEOUT == 90

    def test_api_key_from_api_keys_list(self, monkeypatch):
        """Test that API_KEY extracts first value from API_KEYS comma-separated list."""
        monkeypatch.delenv("API_KEY", raising=False)
        monkeypatch.setenv("API_KEYS", "key1,key2,key3")

        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_KEY

        assert API_KEY == "key1"

    def test_api_key_prefers_api_key_over_api_keys(self, monkeypatch):
        """Test that API_KEY env var takes precedence over API_KEYS."""
        monkeypatch.setenv("API_KEY", "primary-key")
        monkeypatch.setenv("API_KEYS", "fallback-key")

        if 'components.config' in sys.modules:
            del sys.modules['components.config']

        from components.config import API_KEY

        assert API_KEY == "primary-key"
