"""
ML Price Forecast Component

PatchTST direction prediction with confidence bands.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class MLPriceForecastComponent:

    @staticmethod
    def fetch_data(symbol: str, horizon: int = 5, days: int = 90) -> Optional[Dict]:
        try:
            resp = requests.get(
                f"{API_BASE}/api/ml/price-forecast/{symbol}",
                params={"horizon": horizon, "days": days},
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
            print(f"ML price forecast error: {e}")
            return None

    @staticmethod
    def create_forecast_chart(data: Dict) -> go.Figure:
        predicted = data.get("predicted_prices", [])
        current = data.get("current_price")
        direction = data.get("direction", "neutral")

        if not predicted or current is None:
            fig = go.Figure()
            fig.update_layout(height=220, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        color_map = {"up": "#00cc96", "down": "#ef553b", "neutral": "#636efa"}
        line_color = color_map.get(direction, "#636efa")

        # Build x-axis: day 0 = current, day 1..N = forecast
        x_vals = list(range(len(predicted) + 1))
        y_vals = [current] + predicted

        fig = go.Figure()

        # Current price point
        fig.add_trace(go.Scatter(
            x=[0], y=[current], mode="markers",
            marker=dict(size=10, color="white"),
            name="Current",
        ))

        # Forecast line (dashed)
        fig.add_trace(go.Scatter(
            x=x_vals, y=y_vals, mode="lines+markers",
            line=dict(color=line_color, width=2, dash="dash"),
            marker=dict(size=5),
            name=f"Forecast ({direction})",
        ))

        # Confidence band (simple +/- 2% per step)
        upper = [current]
        lower = [current]
        for i, p in enumerate(predicted, 1):
            band = current * 0.02 * i
            upper.append(p + band)
            lower.append(p - band)

        fig.add_trace(go.Scatter(
            x=x_vals + x_vals[::-1],
            y=upper + lower[::-1],
            fill="toself",
            fillcolor=f"rgba({','.join(str(int(c)) for c in go.colors.hex_to_rgb(line_color))},0.1)" if hasattr(go.colors, 'hex_to_rgb') else "rgba(99,110,250,0.1)",
            line=dict(width=0),
            showlegend=False,
        ))

        fig.update_layout(
            title=f"Price Forecast ({len(predicted)} steps)",
            xaxis_title="Days Ahead",
            yaxis_title="Price ($)",
            height=220, margin=dict(t=35, b=30, l=50, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white",
            legend=dict(orientation="h", yanchor="bottom", y=1.02, font=dict(size=9)),
        )
        return fig

    @staticmethod
    def create_direction_bars(data: Dict) -> go.Figure:
        probs = data.get("probabilities", {})
        up = probs.get("up", 0)
        down = probs.get("down", 0)
        neutral = probs.get("neutral", 0)

        fig = go.Figure(go.Bar(
            x=["Up", "Down", "Neutral"],
            y=[up * 100, down * 100, neutral * 100],
            marker_color=["#00cc96", "#ef553b", "#636efa"],
            text=[f"{up*100:.1f}%", f"{down*100:.1f}%", f"{neutral*100:.1f}%"],
            textposition="outside",
        ))
        fig.update_layout(
            title="Direction Probabilities",
            yaxis=dict(range=[0, 100], title="%"),
            height=220, margin=dict(t=35, b=30, l=40, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white",
        )
        return fig

    @staticmethod
    def create_panel(data: Optional[Dict], symbol: str) -> dbc.Card:
        if not data:
            return dbc.Card(dbc.CardBody([
                html.H6("ML Price Forecast", className="card-title"),
                html.P("No forecast data available", className="text-muted small"),
            ]), className="h-100")

        if data.get("_unavailable"):
            return dbc.Card(dbc.CardBody([
                html.H6("ML Price Forecast", className="card-title"),
                html.P(data.get("_message", "Model not loaded"), className="text-warning small"),
            ]), className="h-100")

        direction = data.get("direction", "neutral")
        confidence = data.get("confidence", 0)
        backend = data.get("backend", "unknown")

        dir_color = {"up": "success", "down": "danger", "neutral": "info"}.get(direction, "secondary")

        return dbc.Card([
            dbc.CardHeader([
                html.H6([
                    "ML Price Forecast ",
                    dbc.Badge(direction.upper(), color=dir_color, className="ms-2"),
                    dbc.Badge(f"{confidence:.0%}", color="info", className="ms-2"),
                    dbc.Badge(backend, color="info", className="ms-2", style={"fontSize": "0.65rem"}),
                ], className="mb-0"),
            ]),
            dbc.CardBody([
                dbc.Row([
                    dbc.Col(dcc.Graph(
                        figure=MLPriceForecastComponent.create_forecast_chart(data),
                        config={"displayModeBar": False},
                    ), md=8),
                    dbc.Col(dcc.Graph(
                        figure=MLPriceForecastComponent.create_direction_bars(data),
                        config={"displayModeBar": False},
                    ), md=4),
                ]),
            ]),
        ], className="h-100")
