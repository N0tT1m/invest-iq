"""News Sentiment Component (powered by Polygon News)"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class SocialSentimentComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/sentiment/{symbol}/social",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching social sentiment: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data or not data.get("available"):
            msg = data.get("message", "News sentiment not available") if data else "News sentiment unavailable"
            return dbc.Card([
                dbc.CardHeader(html.H5("News Sentiment", className="mb-0")),
                dbc.CardBody([
                    html.P(msg, className="text-muted"),
                    html.Hr(),
                    html.Small("Analyzes recent news articles for sentiment signals.", className="text-muted"),
                ])
            ])

        sentiment_score = data.get("sentiment_score", 0)
        sentiment_label = data.get("sentiment_label", "Neutral")
        news_mentions = data.get("news_mentions", 0)
        buzz_score = data.get("buzz_score", 0)
        positive_pct = data.get("positive_pct", 0)
        negative_pct = data.get("negative_pct", 0)
        neutral_pct = data.get("neutral_pct", 100)
        headlines = data.get("top_headlines", [])

        # Badges
        score_color = "success" if sentiment_score > 10 else ("danger" if sentiment_score < -10 else "warning")
        label_color = {"Bullish": "success", "Bearish": "danger", "Neutral": "warning"}.get(sentiment_label, "secondary")

        badges = [
            dbc.Badge(f"{sentiment_label}", color=label_color, className="me-2 fs-6"),
            dbc.Badge(f"Score: {sentiment_score:+.1f}", color=score_color, className="me-2"),
            dbc.Badge(f"Articles: {news_mentions}", color="info", className="me-2"),
            dbc.Badge(f"Buzz: {buzz_score:.0f}%", color="warning" if buzz_score > 150 else "secondary", className="me-2"),
        ]

        children = [html.Div(badges, className="mb-3")]

        # Sentiment breakdown bar (horizontal stacked)
        if positive_pct + negative_pct + neutral_pct > 0:
            fig = go.Figure()
            fig.add_trace(go.Bar(
                y=["Sentiment"], x=[positive_pct], orientation='h',
                name='Positive', marker_color='#00cc66',
                text=[f"+{positive_pct:.0f}%"], textposition='inside',
            ))
            fig.add_trace(go.Bar(
                y=["Sentiment"], x=[neutral_pct], orientation='h',
                name='Neutral', marker_color='#666666',
                text=[f"{neutral_pct:.0f}%"], textposition='inside',
            ))
            fig.add_trace(go.Bar(
                y=["Sentiment"], x=[negative_pct], orientation='h',
                name='Negative', marker_color='#ff4444',
                text=[f"-{negative_pct:.0f}%"], textposition='inside',
            ))
            fig.update_layout(
                barmode='stack', height=60, template='plotly_dark',
                margin=dict(l=10, r=10, t=5, b=5),
                showlegend=False, yaxis_visible=False,
                xaxis=dict(range=[0, 100], visible=False),
            )
            children.append(dcc.Graph(figure=fig, config={'displayModeBar': False}))

        # Headlines list
        if headlines:
            children.append(html.H6("Recent Headlines", className="mt-3 mb-2"))
            for h in headlines:
                s = h.get("sentiment", 0)
                label = h.get("sentiment_label", "Neutral")
                dot_color = "#00cc66" if label == "Positive" else ("#ff4444" if label == "Negative" else "#999999")
                children.append(html.Div([
                    html.Span("\u25cf ", style={"color": dot_color, "fontSize": "12px"}),
                    html.A(
                        h.get("title", ""),
                        href=h.get("url", "#"),
                        target="_blank",
                        className="text-light text-decoration-none small",
                        style={"lineHeight": "1.3"},
                    ),
                    html.Span(f" ({h.get('published', '')})", className="text-muted", style={"fontSize": "11px"}),
                ], className="mb-1"))

        # Source note
        if data.get("message"):
            children.append(html.Hr())
            children.append(html.Small(data["message"], className="text-muted"))

        return dbc.Card([
            dbc.CardHeader(html.H5("News Sentiment", className="mb-0")),
            dbc.CardBody(children)
        ])
