"""
ML Trade Signal Component

Meta-model prediction: probability gauge, feature importance, recommendation badge.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class MLTradeSignalComponent:

    @staticmethod
    def fetch_data(symbol: str) -> Optional[Dict]:
        try:
            resp = requests.get(
                f"{API_BASE}/api/ml/trade-signal/{symbol}",
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
            print(f"ML trade signal error: {e}")
            return None

    @staticmethod
    def create_probability_gauge(data: Dict) -> go.Figure:
        prob = data.get("probability", 0.5)
        rec = data.get("recommendation", "SKIP")

        color = "#00cc96" if rec == "EXECUTE" else "#ef553b" if prob < 0.4 else "#636efa"

        fig = go.Figure(go.Indicator(
            mode="gauge+number",
            value=prob * 100,
            title={"text": "Trade Probability"},
            number={"suffix": "%", "font": {"size": 28}},
            gauge={
                "axis": {"range": [0, 100]},
                "bar": {"color": color},
                "steps": [
                    {"range": [0, 40], "color": "rgba(239,85,59,0.15)"},
                    {"range": [40, 60], "color": "rgba(99,110,250,0.15)"},
                    {"range": [60, 100], "color": "rgba(0,204,150,0.15)"},
                ],
                "threshold": {
                    "line": {"color": "white", "width": 2},
                    "thickness": 0.75,
                    "value": prob * 100,
                },
            },
        ))
        fig.update_layout(height=220, margin=dict(t=40, b=10, l=30, r=30),
                          paper_bgcolor="rgba(0,0,0,0)", font_color="white")
        return fig

    @staticmethod
    def create_feature_chart(data: Dict) -> go.Figure:
        features = data.get("features_used", {})
        # Show top 8 features by absolute value
        sorted_feats = sorted(features.items(), key=lambda x: abs(x[1]), reverse=True)[:8]
        if not sorted_feats:
            fig = go.Figure()
            fig.update_layout(height=200, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        names = [f[0].replace("_", " ").title() for f in sorted_feats]
        values = [f[1] for f in sorted_feats]
        colors = ["#00cc96" if v > 0 else "#ef553b" for v in values]

        fig = go.Figure(go.Bar(
            x=values, y=names, orientation="h",
            marker_color=colors,
        ))
        fig.update_layout(
            title="Feature Values",
            height=220, margin=dict(t=35, b=10, l=120, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white", yaxis={"autorange": "reversed"},
        )
        return fig

    @staticmethod
    def create_panel(data: Optional[Dict], symbol: str) -> dbc.Card:
        if not data:
            return dbc.Card(dbc.CardBody([
                html.H6("ML Trade Signal", className="card-title"),
                html.P("No ML signal data available", className="text-muted small"),
            ]), className="h-100")

        if data.get("_unavailable"):
            return dbc.Card(dbc.CardBody([
                html.H6("ML Trade Signal", className="card-title"),
                html.P(data.get("_message", "Model not loaded"), className="text-warning small"),
            ]), className="h-100")

        rec = data.get("recommendation", "SKIP")
        exp_ret = data.get("expected_return", 0)
        backend = data.get("backend", "unknown")

        rec_color = "success" if rec == "EXECUTE" else "danger"

        from dash import dcc
        return dbc.Card([
            dbc.CardHeader([
                html.H6([
                    "ML Trade Signal ",
                    dbc.Badge(rec, color=rec_color, className="ms-2"),
                    dbc.Badge(backend, color="info", className="ms-2", style={"fontSize": "0.65rem"}),
                ], className="mb-0"),
            ]),
            dbc.CardBody([
                dbc.Row([
                    dbc.Col(dcc.Graph(
                        figure=MLTradeSignalComponent.create_probability_gauge(data),
                        config={"displayModeBar": False},
                    ), md=5),
                    dbc.Col(dcc.Graph(
                        figure=MLTradeSignalComponent.create_feature_chart(data),
                        config={"displayModeBar": False},
                    ), md=7),
                ]),
                html.P(
                    f"Expected Return: {exp_ret:+.2f}%",
                    className="text-center text-muted small mb-0 mt-1",
                ),
            ]),
        ], className="h-100")
