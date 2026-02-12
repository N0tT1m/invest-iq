"""Tests for risk_radar component."""
import pytest
import responses
import sys

# Ensure config module is loaded with test env vars before importing component
if 'components.config' in sys.modules:
    del sys.modules['components.config']


class TestRiskRadarComponent:
    """Test suite for RiskRadarComponent."""

    @pytest.fixture(autouse=True)
    def setup(self, set_api_env_vars):
        """Setup for each test - reload modules with test env vars."""
        # Reload config to pick up test env vars
        if 'components.config' in sys.modules:
            del sys.modules['components.config']
        if 'components.risk_radar' in sys.modules:
            del sys.modules['components.risk_radar']

    def test_calculate_risk_from_analysis_with_full_data(self, mock_analysis_response):
        """Test risk calculation with complete analysis data."""
        from components.risk_radar import RiskRadarComponent

        analysis = mock_analysis_response["data"]
        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(analysis)

        # Should return all 6 risk dimensions
        assert len(risk_scores) == 6
        assert "market_risk" in risk_scores
        assert "volatility_risk" in risk_scores
        assert "liquidity_risk" in risk_scores
        assert "event_risk" in risk_scores
        assert "concentration_risk" in risk_scores
        assert "sentiment_risk" in risk_scores

        # All scores should be between 0 and 100
        for key, score in risk_scores.items():
            assert 0.0 <= score <= 100.0, f"{key} score {score} out of range"

        # Market risk should reflect beta (1.15 in mock data)
        # beta * 50 = 57.5
        assert 50 <= risk_scores["market_risk"] <= 65

        # Volatility risk should reflect volatility (22.0% in mock data) and max_drawdown (-15.0%)
        # step1: 22.0 * 2 = 44.0, step2: 44.0 * 0.6 + 15.0 * 2 * 0.4 = 26.4 + 12.0 = 38.4
        assert 35 <= risk_scores["volatility_risk"] <= 42

        # Sentiment risk should reflect confidence (0.68 in mock)
        # (1 - 0.68) * 100 * 0.5 + article_balance_component = ~44
        assert 40 <= risk_scores["sentiment_risk"] <= 50

    def test_calculate_risk_from_analysis_with_empty_dict(self):
        """Test risk calculation with empty analysis dict returns defaults."""
        from components.risk_radar import RiskRadarComponent

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis({})

        # Should return default scores
        assert risk_scores["market_risk"] == 50.0
        assert risk_scores["volatility_risk"] == 50.0
        assert risk_scores["liquidity_risk"] == 30.0
        assert risk_scores["event_risk"] == 30.0
        assert risk_scores["concentration_risk"] == 50.0
        assert risk_scores["sentiment_risk"] == 50.0

    def test_calculate_risk_handles_quant_key_fallback(self):
        """Test that calculation tries both 'quantitative' and 'quant' keys."""
        from components.risk_radar import RiskRadarComponent

        # Test with 'quant' key instead of 'quantitative'
        analysis_with_quant = {
            "quant": {
                "metrics": {
                    "beta": 1.5,
                    "volatility": 0.3
                }
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(analysis_with_quant)

        # Should calculate from 'quant' key
        # beta 1.5 * 50 = 75
        assert 70 <= risk_scores["market_risk"] <= 80

    def test_calculate_risk_clamps_to_valid_range(self):
        """Test that risk scores are clamped to 0-100 range."""
        from components.risk_radar import RiskRadarComponent

        # Extreme values that would exceed range (percentages)
        extreme_analysis = {
            "quantitative": {
                "metrics": {
                    "beta": 5.0,  # Would give 250 without clamping
                    "volatility": 60.0,  # 60% vol, 60*2=120 clamped to 100
                    "max_drawdown": -40.0  # -40%
                }
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(extreme_analysis)

        # Market risk should be clamped to 100
        assert risk_scores["market_risk"] == 100.0
        # Volatility risk is blended: min(100, 60*2)*0.6 + abs(-40)*2*0.4 = 60 + 32 = 92
        assert risk_scores["volatility_risk"] == 92.0

    @responses.activate
    def test_fetch_risk_radar_success(self):
        """Test successful fetch of risk radar data from API."""
        from components.risk_radar import RiskRadarComponent

        mock_response = {
            "success": True,
            "data": {
                "market_risk": 60.0,
                "volatility_risk": 45.0,
                "liquidity_risk": 25.0,
                "event_risk": 30.0,
                "concentration_risk": 50.0,
                "sentiment_risk": 40.0
            }
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar/AAPL",
            json=mock_response,
            status=200
        )

        result = RiskRadarComponent.fetch_risk_radar("AAPL")

        assert result is not None
        assert result["market_risk"] == 60.0
        assert result["volatility_risk"] == 45.0

    @responses.activate
    def test_fetch_risk_radar_failure(self):
        """Test fetch_risk_radar returns None on HTTP 500."""
        from components.risk_radar import RiskRadarComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar/AAPL",
            json={"success": False, "error": "Internal server error"},
            status=500
        )

        result = RiskRadarComponent.fetch_risk_radar("AAPL")

        assert result is None

    @responses.activate
    def test_fetch_risk_radar_unsuccessful_response(self):
        """Test fetch_risk_radar returns None when success=false."""
        from components.risk_radar import RiskRadarComponent

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar/AAPL",
            json={"success": False, "error": "Symbol not found"},
            status=200
        )

        result = RiskRadarComponent.fetch_risk_radar("AAPL")

        assert result is None

    @responses.activate
    def test_fetch_risk_radar_without_symbol(self):
        """Test fetch_risk_radar without symbol uses portfolio endpoint."""
        from components.risk_radar import RiskRadarComponent

        mock_response = {
            "success": True,
            "data": {
                "market_risk": 50.0,
                "volatility_risk": 40.0,
                "liquidity_risk": 30.0,
                "event_risk": 35.0,
                "concentration_risk": 60.0,
                "sentiment_risk": 45.0
            }
        }

        responses.add(
            responses.GET,
            "http://localhost:3000/api/risk/radar",
            json=mock_response,
            status=200
        )

        result = RiskRadarComponent.fetch_risk_radar()

        assert result is not None
        assert result["concentration_risk"] == 60.0

    def test_create_risk_card_with_none_data(self):
        """Test create_risk_card handles None data gracefully."""
        from components.risk_radar import RiskRadarComponent

        card = RiskRadarComponent.create_risk_card(None)

        # Should return a card (not None or empty)
        assert card is not None

    def test_create_risk_card_with_valid_data(self):
        """Test create_risk_card with valid risk data."""
        from components.risk_radar import RiskRadarComponent

        risk_data = {
            "market_risk": 60.0,
            "volatility_risk": 75.0,
            "liquidity_risk": 25.0,
            "event_risk": 30.0,
            "concentration_risk": 45.0,
            "sentiment_risk": 50.0
        }

        card = RiskRadarComponent.create_risk_card(risk_data, "AAPL")

        # Should return a valid card component
        assert card is not None

    def test_calculate_risk_with_sentiment_article_distribution(self):
        """Test sentiment risk calculation with article distribution."""
        from components.risk_radar import RiskRadarComponent

        # Balanced sentiment (mixed = higher risk)
        balanced_analysis = {
            "sentiment": {
                "confidence": 0.5,
                "metrics": {
                    "positive_articles": 10,
                    "negative_articles": 10,
                    "total_articles": 20
                }
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(balanced_analysis)

        # Mixed sentiment should increase risk
        # (1 - 0.5) * 100 = 50 base, balance = abs(10-10)/20 = 0
        # Total = 50 * 0.5 + (1 - 0) * 100 * 0.5 = 25 + 50 = 75
        assert 70 <= risk_scores["sentiment_risk"] <= 80

        # Strongly biased sentiment (lower risk)
        biased_analysis = {
            "sentiment": {
                "confidence": 0.8,
                "metrics": {
                    "positive_articles": 18,
                    "negative_articles": 2,
                    "total_articles": 20
                }
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(biased_analysis)

        # Strong bias should lower risk
        # (1 - 0.8) * 100 = 20 base, balance = abs(18-2)/20 = 0.8
        # Total = 20 * 0.5 + (1 - 0.8) * 100 * 0.5 = 10 + 10 = 20
        assert risk_scores["sentiment_risk"] < 25

    def test_calculate_risk_event_risk_from_technical_confidence(self):
        """Test event risk calculation from technical confidence."""
        from components.risk_radar import RiskRadarComponent

        high_confidence_tech = {
            "technical": {
                "confidence": 0.9
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(high_confidence_tech)

        # (1 - 0.9) * 60 + 20 = 6 + 20 = 26
        assert 20 <= risk_scores["event_risk"] <= 30

        low_confidence_tech = {
            "technical": {
                "confidence": 0.3
            }
        }

        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(low_confidence_tech)

        # (1 - 0.3) * 60 + 20 = 42 + 20 = 62
        assert 60 <= risk_scores["event_risk"] <= 65
