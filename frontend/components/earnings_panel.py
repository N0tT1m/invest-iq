"""Earnings Analysis Panel Component"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class EarningsPanelComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/earnings/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching earnings: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data:
            return dbc.Card([
                dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
                dbc.CardBody(html.P("No earnings data available", className="text-muted"))
            ])

        data_source = data.get("data_source", "premium")
        historical = data.get("historical", [])
        earnings_news = data.get("earnings_news", [])

        # News fallback view
        if data_source == "news_fallback":
            if not earnings_news:
                return dbc.Card([
                    dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
                    dbc.CardBody([
                        html.P("Earnings financials require a premium Polygon.io plan.", className="text-muted"),
                        html.Small("No earnings-related news found for this symbol.", className="text-muted"),
                    ])
                ], className="h-100")

            children = [
                dbc.Badge("News Fallback", color="warning", className="mb-2"),
                html.P(
                    "Earnings financials require a premium plan. Showing earnings-related news:",
                    className="text-muted small mb-2"
                ),
            ]
            for article in earnings_news:
                children.append(html.Div([
                    html.Span("\u25cf ", style={"color": "#00aaff", "fontSize": "10px"}),
                    html.A(
                        article.get("title", ""),
                        href=article.get("url", "#"),
                        target="_blank",
                        className="text-light text-decoration-none small",
                    ),
                    html.Span(f" ({article.get('published', '')})", className="text-muted", style={"fontSize": "11px"}),
                ], className="mb-1"))

            return dbc.Card([
                dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
                dbc.CardBody(children)
            ], className="h-100")

        # Full premium view
        if not historical:
            return dbc.Card([
                dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
                dbc.CardBody(html.P("No earnings data available", className="text-muted"))
            ])

        eps_growth = data.get("eps_growth_rate")
        rev_growth = data.get("revenue_growth_rate")
        beat_rate = data.get("beat_rate")

        # EPS bar chart
        eps_fig = go.Figure()
        quarters = [f"{q['fiscal_period']} {q['fiscal_year']}" for q in historical]
        eps_values = [q.get("eps") or 0 for q in historical]
        colors = ['#00cc66' if v >= 0 else '#ff4444' for v in eps_values]

        eps_fig.add_trace(go.Bar(
            x=quarters, y=eps_values,
            marker_color=colors,
            name="EPS",
            hovertemplate="EPS: $%{y:.2f}<extra></extra>"
        ))
        eps_fig.update_layout(
            height=250, template='plotly_dark',
            margin=dict(l=40, r=20, t=30, b=40),
            title=dict(text="EPS by Quarter", font=dict(size=13)),
            yaxis_title="EPS ($)",
        )

        # Revenue trend
        rev_fig = go.Figure()
        rev_values = [q.get("revenue") for q in historical]
        rev_labels = [q.get("revenue_formatted", "N/A") for q in historical]
        has_revenue = any(v is not None for v in rev_values)

        if has_revenue:
            rev_fig.add_trace(go.Scatter(
                x=quarters,
                y=[v or 0 for v in rev_values],
                mode='lines+markers',
                line=dict(color='#00aaff', width=2),
                name="Revenue",
                hovertext=rev_labels,
                hoverinfo='text+x',
            ))
            rev_fig.update_layout(
                height=200, template='plotly_dark',
                margin=dict(l=40, r=20, t=30, b=40),
                title=dict(text="Revenue Trend", font=dict(size=13)),
                yaxis_title="Revenue",
            )

        # Badges
        badges = []
        if eps_growth is not None:
            color = "success" if eps_growth > 0 else "danger"
            badges.append(dbc.Badge(f"EPS Growth: {eps_growth:+.1f}%", color=color, className="me-2"))
        if rev_growth is not None:
            color = "success" if rev_growth > 0 else "danger"
            badges.append(dbc.Badge(f"Revenue Growth: {rev_growth:+.1f}%", color=color, className="me-2"))
        if beat_rate is not None:
            color = "success" if beat_rate >= 50 else "warning"
            badges.append(dbc.Badge(f"Beat Rate: {beat_rate:.0f}%", color=color, className="me-2"))

        body_children = [
            html.Div(badges, className="mb-3") if badges else html.Div(),
            dcc.Graph(figure=eps_fig, config={'displayModeBar': False}),
        ]
        if has_revenue:
            body_children.append(dcc.Graph(figure=rev_fig, config={'displayModeBar': False}))

        return dbc.Card([
            dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
            dbc.CardBody(body_children)
        ], className="h-100")
