"""Dividend Analysis Panel Component"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html

from components.config import API_BASE, get_headers, API_TIMEOUT


class DividendPanelComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/dividends/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching dividends: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data:
            return dbc.Card([
                dbc.CardHeader(html.H5("Dividend Analysis", className="mb-0")),
                dbc.CardBody(html.P("No dividend data available", className="text-muted"))
            ], className="h-100")

        data_source = data.get("data_source", "premium")
        has_dividends = data.get("has_dividends", False)
        dividend_news = data.get("dividend_news", [])
        message = data.get("message")

        # News fallback view
        if data_source == "news_fallback":
            children = []
            if message:
                children.append(html.P(message, className="text-muted small"))

            if dividend_news:
                children.append(dbc.Badge("News Fallback", color="warning", className="mb-2"))
                for article in dividend_news:
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
            elif not children:
                children.append(html.P("Dividend data requires a premium Polygon.io plan.", className="text-muted"))

            return dbc.Card([
                dbc.CardHeader(html.H5("Dividend Analysis", className="mb-0")),
                dbc.CardBody(children)
            ], className="h-100")

        # Premium: no dividends
        if not has_dividends:
            return dbc.Card([
                dbc.CardHeader(html.H5("Dividend Analysis", className="mb-0")),
                dbc.CardBody(html.P("This stock does not pay dividends", className="text-muted"))
            ], className="h-100")

        # Premium: full view
        current_yield = data.get("current_yield")
        annual_div = data.get("annual_dividend")
        frequency = data.get("frequency", "N/A")
        ex_date = data.get("ex_dividend_date", "N/A")
        pay_date = data.get("pay_date", "N/A")
        growth_rate = data.get("growth_rate")
        history = data.get("history", [])

        # Badges
        badges = []
        if current_yield is not None:
            color = "success" if current_yield >= 2.0 else "info"
            badges.append(dbc.Badge(f"Yield: {current_yield:.2f}%", color=color, className="me-2 fs-6"))
        if annual_div is not None:
            badges.append(dbc.Badge(f"Annual: ${annual_div:.2f}", color="primary", className="me-2"))
        if frequency:
            badges.append(dbc.Badge(frequency, color="secondary", className="me-2"))
        if growth_rate is not None:
            color = "success" if growth_rate > 0 else "danger"
            badges.append(dbc.Badge(f"Growth: {growth_rate:+.1f}%", color=color, className="me-2"))

        # Date info
        date_info = []
        if ex_date and ex_date != "N/A":
            date_info.append(html.Small(f"Ex-Dividend: {ex_date}", className="text-muted me-3"))
        if pay_date and pay_date != "N/A":
            date_info.append(html.Small(f"Pay Date: {pay_date}", className="text-muted"))

        body_children = [
            html.Div(badges, className="mb-2"),
            html.Div(date_info, className="mb-3") if date_info else html.Div(),
        ]

        if history:
            from dash import dcc
            fig = go.Figure()
            dates = [h["date"] for h in history]
            amounts = [h["amount"] for h in history]
            fig.add_trace(go.Bar(
                x=dates, y=amounts,
                marker_color='#00cc66',
                name="Dividend",
                hovertemplate="$%{y:.4f}<extra></extra>"
            ))
            fig.update_layout(
                height=200, template='plotly_dark',
                margin=dict(l=40, r=20, t=30, b=40),
                title=dict(text="Dividend History", font=dict(size=13)),
                yaxis_title="Amount ($)",
            )
            body_children.append(dcc.Graph(figure=fig, config={'displayModeBar': False}))

        return dbc.Card([
            dbc.CardHeader(html.H5("Dividend Analysis", className="mb-0")),
            dbc.CardBody(body_children)
        ], className="h-100")
