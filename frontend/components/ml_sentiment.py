"""
ML Sentiment Component

FinBERT NLP sentiment analysis on news headlines.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, Optional, List
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class MLSentimentComponent:

    @staticmethod
    def fetch_data(symbol: str) -> Optional[Dict]:
        try:
            resp = requests.get(
                f"{API_BASE}/api/ml/sentiment/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if resp.status_code == 200:
                data = resp.json()
                if data.get("success"):
                    return data.get("data")
            if resp.status_code == 503:
                body = resp.json()
                return {"_unavailable": True, "_message": body.get("error", "Model not loaded")}
            return None
        except Exception as e:
            print(f"ML sentiment error: {e}")
            return None

    @staticmethod
    def create_sentiment_gauge(data: Dict) -> go.Figure:
        score = data.get("score", 0)
        overall = data.get("overall_sentiment", "neutral")

        color_map = {"positive": "#00cc96", "negative": "#ef553b", "neutral": "#636efa"}
        color = color_map.get(overall, "#636efa")

        fig = go.Figure(go.Indicator(
            mode="gauge+number",
            value=score,
            title={"text": f"Overall: {overall.title()}"},
            number={"font": {"size": 24}},
            gauge={
                "axis": {"range": [-1, 1]},
                "bar": {"color": color},
                "steps": [
                    {"range": [-1, -0.3], "color": "rgba(239,85,59,0.15)"},
                    {"range": [-0.3, 0.3], "color": "rgba(99,110,250,0.15)"},
                    {"range": [0.3, 1], "color": "rgba(0,204,150,0.15)"},
                ],
            },
        ))
        fig.update_layout(height=200, margin=dict(t=40, b=10, l=30, r=30),
                          paper_bgcolor="rgba(0,0,0,0)", font_color="white")
        return fig

    @staticmethod
    def create_distribution_pie(data: Dict) -> go.Figure:
        pos = data.get("positive_ratio", 0)
        neg = data.get("negative_ratio", 0)
        neu = data.get("neutral_ratio", 0)

        fig = go.Figure(go.Pie(
            values=[pos, neg, neu],
            labels=["Positive", "Negative", "Neutral"],
            marker=dict(colors=["#00cc96", "#ef553b", "#636efa"]),
            hole=0.4,
            textinfo="percent+label",
            textfont=dict(size=10),
        ))
        fig.update_layout(height=200, margin=dict(t=10, b=10, l=10, r=10),
                          paper_bgcolor="rgba(0,0,0,0)", font_color="white",
                          showlegend=False)
        return fig

    @staticmethod
    def create_article_bars(articles: List[Dict]) -> go.Figure:
        if not articles:
            fig = go.Figure()
            fig.update_layout(height=200, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        # Show up to 10 articles
        articles = articles[:10]
        headlines = [a.get("headline", "")[:40] + "..." for a in articles]
        pos_vals = [a.get("positive", 0) for a in articles]
        neg_vals = [a.get("negative", 0) for a in articles]
        neu_vals = [a.get("neutral", 0) for a in articles]

        fig = go.Figure()
        fig.add_trace(go.Bar(y=headlines, x=pos_vals, name="Positive",
                             orientation="h", marker_color="#00cc96"))
        fig.add_trace(go.Bar(y=headlines, x=neg_vals, name="Negative",
                             orientation="h", marker_color="#ef553b"))
        fig.add_trace(go.Bar(y=headlines, x=neu_vals, name="Neutral",
                             orientation="h", marker_color="#636efa"))
        fig.update_layout(
            barmode="stack",
            title="Per-Article Sentiment",
            height=max(200, len(articles) * 25 + 60),
            margin=dict(t=35, b=10, l=200, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white", font_size=9,
            yaxis={"autorange": "reversed"},
            legend=dict(orientation="h", yanchor="bottom", y=1.02, font=dict(size=9)),
        )
        return fig

    @staticmethod
    def create_panel(data: Optional[Dict], symbol: str) -> dbc.Card:
        if not data:
            return dbc.Card(dbc.CardBody([
                html.H6("ML Sentiment (FinBERT)", className="card-title"),
                html.P("No sentiment data available", className="text-muted small"),
            ]), className="h-100")

        if data.get("_unavailable"):
            return dbc.Card(dbc.CardBody([
                html.H6("ML Sentiment (FinBERT)", className="card-title"),
                html.P(data.get("_message", "Model not loaded"), className="text-warning small"),
            ]), className="h-100")

        articles = data.get("articles", [])
        article_count = data.get("article_count", 0)
        confidence = data.get("confidence", 0)
        backend = data.get("backend", "unknown")

        return dbc.Card([
            dbc.CardHeader([
                html.H6([
                    "ML Sentiment (FinBERT) ",
                    dbc.Badge(f"{article_count} articles", color="secondary", className="ms-2"),
                    dbc.Badge(f"{confidence:.0%} conf", color="info", className="ms-2"),
                    dbc.Badge(backend, color="info", className="ms-2", style={"fontSize": "0.65rem"}),
                ], className="mb-0"),
            ]),
            dbc.CardBody([
                dbc.Row([
                    dbc.Col(dcc.Graph(
                        figure=MLSentimentComponent.create_sentiment_gauge(data),
                        config={"displayModeBar": False},
                    ), md=4),
                    dbc.Col(dcc.Graph(
                        figure=MLSentimentComponent.create_distribution_pie(data),
                        config={"displayModeBar": False},
                    ), md=3),
                    dbc.Col(dcc.Graph(
                        figure=MLSentimentComponent.create_article_bars(articles),
                        config={"displayModeBar": False},
                    ), md=5),
                ]),
            ]),
        ], className="h-100")
