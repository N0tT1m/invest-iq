"""Load testing for InvestIQ API using Locust.

Usage:
    locust -f locustfile.py --host=http://localhost:3000
    locust -f locustfile.py --host=http://localhost:3000 --headless -u 50 -r 5 -t 60s
"""
import os
import random
from locust import HttpUser, task, between, tag

API_KEY = os.getenv("API_KEY", "test_key")
SYMBOLS = ["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA", "NVDA", "META", "JPM", "V", "JNJ"]
HEADERS = {"X-API-Key": API_KEY, "Content-Type": "application/json"}


class HealthCheckUser(HttpUser):
    """Baseline user that only hits health endpoints."""
    weight = 1
    wait_time = between(0.5, 1)

    @task(3)
    @tag("health")
    def health(self):
        self.client.get("/health", headers=HEADERS)

    @task(1)
    @tag("metrics")
    def metrics(self):
        self.client.get("/metrics", headers=HEADERS)

    @task(1)
    @tag("metrics")
    def metrics_json(self):
        self.client.get("/metrics/json", headers=HEADERS)


class AnalysisUser(HttpUser):
    """User that performs stock analysis — the heaviest endpoint."""
    weight = 3
    wait_time = between(2, 5)

    @task(5)
    @tag("analysis")
    def analyze_symbol(self):
        symbol = random.choice(SYMBOLS)
        self.client.get(f"/api/analyze/{symbol}", headers=HEADERS, name="/api/analyze/:symbol")

    @task(2)
    @tag("bars")
    def get_bars(self):
        symbol = random.choice(SYMBOLS)
        self.client.get(
            f"/api/bars/{symbol}",
            params={"days": 90},
            headers=HEADERS,
            name="/api/bars/:symbol",
        )

    @task(1)
    @tag("ticker")
    def get_ticker(self):
        symbol = random.choice(SYMBOLS)
        self.client.get(f"/api/ticker/{symbol}", headers=HEADERS, name="/api/ticker/:symbol")

    @task(1)
    @tag("backtest")
    def backtest(self):
        symbol = random.choice(SYMBOLS[:5])
        self.client.get(
            f"/api/backtest/{symbol}",
            params={"days": 180},
            headers=HEADERS,
            name="/api/backtest/:symbol",
            timeout=120,
        )


class TradingUser(HttpUser):
    """User that reads trading/portfolio data."""
    weight = 2
    wait_time = between(1, 3)

    @task(3)
    @tag("broker")
    def get_account(self):
        self.client.get("/api/broker/account", headers=HEADERS)

    @task(2)
    @tag("broker")
    def get_positions(self):
        self.client.get("/api/broker/positions", headers=HEADERS)

    @task(1)
    @tag("broker")
    def get_orders(self):
        self.client.get("/api/broker/orders", headers=HEADERS)

    @task(2)
    @tag("portfolio")
    def portfolio_summary(self):
        self.client.get("/api/portfolio/summary", headers=HEADERS)

    @task(1)
    @tag("risk")
    def risk_radar(self):
        symbol = random.choice(SYMBOLS)
        self.client.get(
            f"/api/risk/radar/{symbol}",
            headers=HEADERS,
            name="/api/risk/radar/:symbol",
        )

    @task(1)
    @tag("risk")
    def circuit_breakers(self):
        self.client.get("/api/risk/circuit-breakers", headers=HEADERS)


class MixedUser(HttpUser):
    """Simulates a real user navigating the dashboard."""
    weight = 5
    wait_time = between(1, 4)

    @task(3)
    @tag("analysis")
    def full_analysis_flow(self):
        """Simulates loading the main dashboard for a symbol."""
        symbol = random.choice(SYMBOLS)
        # Main analysis
        self.client.get(f"/api/analyze/{symbol}", headers=HEADERS, name="/api/analyze/:symbol")

    @task(2)
    @tag("health")
    def check_health(self):
        self.client.get("/health", headers=HEADERS)

    @task(2)
    @tag("broker")
    def check_portfolio(self):
        self.client.get("/api/broker/account", headers=HEADERS)
        self.client.get("/api/broker/positions", headers=HEADERS)

    @task(1)
    @tag("bars")
    def view_charts(self):
        symbol = random.choice(SYMBOLS)
        self.client.get(
            f"/api/bars/{symbol}",
            params={"days": 365, "timeframe": "1d"},
            headers=HEADERS,
            name="/api/bars/:symbol",
        )

    @task(1)
    @tag("analysis")
    def suggest_stocks(self):
        self.client.get(
            "/api/suggest",
            params={"universe": "popular", "limit": 5},
            headers=HEADERS,
        )
