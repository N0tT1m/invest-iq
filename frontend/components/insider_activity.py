"""Insider Trading Activity Component"""
import requests
import dash_bootstrap_components as dbc
from dash import html

from components.config import API_BASE, get_headers, API_TIMEOUT


class InsiderActivityComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/insiders/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching insider data: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data:
            return dbc.Card([
                dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
                dbc.CardBody(html.P("No insider data available", className="text-muted"))
            ])

        data_source = data.get("data_source", "premium")
        insider_news = data.get("insider_news", [])
        message = data.get("message")

        # News fallback view
        if data_source == "news_fallback":
            if not data.get("available") and not insider_news:
                return dbc.Card([
                    dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
                    dbc.CardBody([
                        html.P(
                            message or "Insider data requires a premium Polygon.io plan.",
                            className="text-muted"
                        ),
                    ])
                ])

            children = [
                dbc.Badge("News Fallback", color="warning", className="mb-2"),
                html.P(
                    "Insider transaction data requires a premium plan. Showing insider-related news:",
                    className="text-muted small mb-2"
                ),
            ]
            for article in insider_news:
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
                dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
                dbc.CardBody(children)
            ])

        # Premium: full transaction view
        if not data.get("available"):
            msg = message or "No insider data available"
            return dbc.Card([
                dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
                dbc.CardBody(html.P(msg, className="text-muted"))
            ])

        transactions = data.get("transactions", [])
        net_sentiment = data.get("net_sentiment", "N/A")
        total_buys = data.get("total_buys", 0)
        total_sells = data.get("total_sells", 0)
        net_value = data.get("net_value")

        # Sentiment badge
        sentiment_color = {
            "Strongly Bullish": "success",
            "Bullish": "success",
            "Neutral": "warning",
            "Bearish": "danger",
            "Strongly Bearish": "danger",
        }.get(net_sentiment, "secondary")

        badges = [
            dbc.Badge(f"Insider Sentiment: {net_sentiment}", color=sentiment_color, className="me-2 fs-6"),
            dbc.Badge(f"Buys: {total_buys}", color="success", className="me-2"),
            dbc.Badge(f"Sells: {total_sells}", color="danger", className="me-2"),
        ]
        if net_value is not None:
            nv_color = "success" if net_value > 0 else "danger"
            nv_str = f"${abs(net_value):,.0f}" if abs(net_value) < 1e6 else f"${abs(net_value)/1e6:.1f}M"
            prefix = "+" if net_value > 0 else "-"
            badges.append(dbc.Badge(f"Net: {prefix}{nv_str}", color=nv_color, className="me-2"))

        # Transactions table
        rows = []
        for t in transactions[:10]:
            tx_type = t.get("transaction_type", "")
            is_buy = "purchase" in tx_type.lower() or "buy" in tx_type.lower()
            value = t.get("total_value")
            value_str = f"${value:,.0f}" if value else "N/A"

            rows.append(html.Tr([
                html.Td(t.get("date", "")[:10]),
                html.Td(t.get("name", "Unknown")),
                html.Td(t.get("title", "N/A"), className="small"),
                html.Td(
                    "BUY" if is_buy else "SELL",
                    className="text-success fw-bold" if is_buy else "text-danger fw-bold"
                ),
                html.Td(f"{t.get('shares', 0):,.0f}"),
                html.Td(value_str),
            ]))

        table = dbc.Table([
            html.Thead(html.Tr([
                html.Th("Date"), html.Th("Name"), html.Th("Title"),
                html.Th("Type"), html.Th("Shares"), html.Th("Value"),
            ])),
            html.Tbody(rows),
        ], bordered=True, dark=True, hover=True, size="sm", className="mt-2 mb-0") if rows else html.P("No recent transactions", className="text-muted")

        return dbc.Card([
            dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
            dbc.CardBody([
                html.Div(badges, className="mb-3"),
                table,
            ])
        ])
