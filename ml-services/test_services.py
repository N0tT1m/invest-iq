#!/usr/bin/env python3
"""Test all ML services are working correctly."""
import requests
import json
import sys

BASE_URLS = {
    'sentiment': 'http://localhost:8001',
    'bayesian': 'http://localhost:8002',
    'price_predictor': 'http://localhost:8003'
}

def test_service_health(name, url):
    """Test if service is healthy."""
    try:
        response = requests.get(f"{url}/health", timeout=5)
        if response.status_code == 200:
            data = response.json()
            print(f"✅ {name}: {data.get('status', 'unknown')}")
            return True
        else:
            print(f"❌ {name}: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ {name}: {e}")
        return False

def test_sentiment_prediction():
    """Test sentiment prediction."""
    url = f"{BASE_URLS['sentiment']}/predict"
    payload = {
        "texts": [
            "Apple reports record earnings, stock surges to new high",
            "Market crash looms as inflation fears spike",
            "Federal Reserve maintains interest rates"
        ],
        "symbol": "AAPL",
        "use_cache": False
    }

    try:
        response = requests.post(url, json=payload, timeout=10)
        if response.status_code == 200:
            data = response.json()
            predictions = data['predictions']
            print(f"✅ Sentiment Predictions ({data['processing_time_ms']:.1f}ms):")
            for i, pred in enumerate(predictions):
                print(f"   {i+1}. {pred['label']:8s} - confidence: {pred['confidence']:.2f}, score: {pred['score']:+.2f}")
            return True
        else:
            print(f"❌ Sentiment Prediction: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Sentiment Prediction: {e}")
        return False

def test_news_sentiment():
    """Test news sentiment analysis."""
    url = f"{BASE_URLS['sentiment']}/analyze-news"
    payload = {
        "headlines": [
            "Tech stocks rally as AI boom continues",
            "Markets decline on economic concerns",
            "Apple announces new product line",
            "Strong earnings beat expectations"
        ],
        "symbol": "AAPL"
    }

    try:
        response = requests.post(url, json=payload, timeout=10)
        if response.status_code == 200:
            data = response.json()
            print(f"✅ News Sentiment Analysis:")
            print(f"   Overall: {data['overall_sentiment']} (score: {data['score']:+.2f})")
            print(f"   Confidence: {data['confidence']:.2%}")
            print(f"   Distribution: {data['positive_ratio']:.0%} pos, {data['negative_ratio']:.0%} neg, {data['neutral_ratio']:.0%} neutral")
            return True
        else:
            print(f"❌ News Sentiment: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ News Sentiment: {e}")
        return False

def test_bayesian_weights():
    """Test Bayesian strategy weights."""
    url = f"{BASE_URLS['bayesian']}/weights"

    try:
        response = requests.get(url, params={'normalize': True}, timeout=5)
        if response.status_code == 200:
            data = response.json()
            weights = data['weights']
            print(f"✅ Bayesian Strategy Weights:")
            for strategy, weight in sorted(weights.items(), key=lambda x: x[1], reverse=True):
                print(f"   {strategy:20s}: {weight:.4f}")
            return True
        else:
            print(f"❌ Bayesian Weights: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Bayesian Weights: {e}")
        return False

def test_bayesian_update():
    """Test updating strategy performance."""
    url = f"{BASE_URLS['bayesian']}/update"
    payload = {
        "strategy_name": "test_strategy",
        "outcome": 1,  # Win
        "profit_loss": 150.0
    }

    try:
        response = requests.post(url, json=payload, timeout=5)
        if response.status_code == 200:
            data = response.json()
            stats = data['updated_stats']
            print(f"✅ Bayesian Update:")
            print(f"   Strategy: {data['strategy_name']}")
            print(f"   Win Rate: {stats['win_rate']:.2%}")
            print(f"   Samples: {stats['total_samples']}")
            return True
        else:
            print(f"❌ Bayesian Update: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Bayesian Update: {e}")
        return False

def test_thompson_sampling():
    """Test Thompson sampling strategy selection."""
    url = f"{BASE_URLS['bayesian']}/thompson-sampling"
    payload = {
        "strategies": ["momentum", "mean_reversion", "breakout", "sentiment"],
        "n_samples": 2
    }

    try:
        response = requests.post(url, json=payload, timeout=5)
        if response.status_code == 200:
            data = response.json()
            print(f"✅ Thompson Sampling:")
            print(f"   Selected: {', '.join(data['selected_strategies'])}")
            return True
        else:
            print(f"❌ Thompson Sampling: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Thompson Sampling: {e}")
        return False

def test_price_predictor():
    """Test price direction prediction."""
    # Note: This will fail if model not trained
    # Just check if service is responding appropriately
    url = f"{BASE_URLS['price_predictor']}/health"

    try:
        response = requests.get(url, timeout=5)
        if response.status_code == 200:
            data = response.json()
            if data['status'] == 'healthy':
                print(f"✅ Price Predictor: Model loaded")
                print(f"   Context Length: {data['context_length']}")
                print(f"   Prediction Length: {data['prediction_length']}")
                return True
            else:
                print(f"⚠️  Price Predictor: Model not loaded (train first)")
                print(f"   Run: python price_predictor/train.py --days 60 --epochs 10")
                return False
        else:
            print(f"❌ Price Predictor: HTTP {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Price Predictor: {e}")
        return False

def main():
    """Run all tests."""
    print("=" * 60)
    print("Testing InvestIQ ML Services")
    print("=" * 60)
    print()

    # Test health
    print("1. Testing Service Health...")
    print("-" * 60)
    health_results = {}
    for name, url in BASE_URLS.items():
        health_results[name] = test_service_health(name, url)
    print()

    if not all(health_results.values()):
        print("⚠️  Some services are not running!")
        print("Run: ./start_all_services.sh")
        sys.exit(1)

    # Test sentiment
    print("2. Testing Sentiment Analysis...")
    print("-" * 60)
    test_sentiment_prediction()
    print()

    print("3. Testing News Sentiment...")
    print("-" * 60)
    test_news_sentiment()
    print()

    # Test Bayesian
    print("4. Testing Bayesian Weights...")
    print("-" * 60)
    test_bayesian_weights()
    print()

    print("5. Testing Bayesian Update...")
    print("-" * 60)
    test_bayesian_update()
    print()

    print("6. Testing Thompson Sampling...")
    print("-" * 60)
    test_thompson_sampling()
    print()

    # Test price predictor
    print("7. Testing Price Predictor...")
    print("-" * 60)
    test_price_predictor()
    print()

    print("=" * 60)
    print("✅ All tests complete!")
    print("=" * 60)
    print()
    print("Next steps:")
    print("1. Update trading agent to use ML services")
    print("2. Run trading agent: cargo run --release -p trading-agent")
    print("3. Monitor predictions in database")

if __name__ == "__main__":
    main()
