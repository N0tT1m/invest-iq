"""
ML Calibration Component

Raw vs calibrated confidence per engine with reliability tiers.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class MLCalibrationComponent:

    @staticmethod
    def fetch_data(symbol: str) -> Optional[Dict]:
        try:
            resp = requests.get(
                f"{API_BASE}/api/ml/calibration/{symbol}",
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
            print(f"ML calibration error: {e}")
            return None

    @staticmethod
    def create_comparison_chart(engines: Dict) -> go.Figure:
        if not engines:
            fig = go.Figure()
            fig.update_layout(height=220, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        names = [n.replace("_", " ").title() for n in engines.keys()]
        raw_vals = [engines[n].get("raw", 0) * 100 for n in engines]
        cal_vals = [engines[n].get("calibrated", 0) * 100 for n in engines]

        fig = go.Figure()
        fig.add_trace(go.Bar(
            name="Raw", x=names, y=raw_vals,
            marker_color="rgba(99,110,250,0.6)",
            text=[f"{v:.1f}%" for v in raw_vals],
            textposition="outside",
        ))
        fig.add_trace(go.Bar(
            name="Calibrated", x=names, y=cal_vals,
            marker_color="#636efa",
            text=[f"{v:.1f}%" for v in cal_vals],
            textposition="outside",
        ))

        fig.update_layout(
            title="Raw vs Calibrated Confidence",
            barmode="group",
            yaxis=dict(range=[0, 100], title="%"),
            height=240, margin=dict(t=35, b=30, l=40, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white",
            legend=dict(orientation="h", yanchor="bottom", y=1.02, font=dict(size=9)),
        )
        return fig

    @staticmethod
    def create_radar_chart(engines: Dict) -> go.Figure:
        if not engines:
            fig = go.Figure()
            fig.update_layout(height=240, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        categories = [n.replace("_", " ").title() for n in engines.keys()]
        raw_vals = [engines[n].get("raw", 0) * 100 for n in engines]
        cal_vals = [engines[n].get("calibrated", 0) * 100 for n in engines]

        # Close the polygon
        categories_closed = categories + [categories[0]]
        raw_closed = raw_vals + [raw_vals[0]]
        cal_closed = cal_vals + [cal_vals[0]]

        fig = go.Figure()
        fig.add_trace(go.Scatterpolar(
            r=raw_closed, theta=categories_closed,
            fill="toself", fillcolor="rgba(99,110,250,0.1)",
            line=dict(color="rgba(99,110,250,0.5)", dash="dash"),
            name="Raw",
        ))
        fig.add_trace(go.Scatterpolar(
            r=cal_closed, theta=categories_closed,
            fill="toself", fillcolor="rgba(0,204,150,0.1)",
            line=dict(color="#00cc96"),
            name="Calibrated",
        ))

        fig.update_layout(
            polar=dict(
                radialaxis=dict(visible=True, range=[0, 100]),
                bgcolor="rgba(0,0,0,0)",
            ),
            height=240, margin=dict(t=20, b=20, l=40, r=40),
            paper_bgcolor="rgba(0,0,0,0)",
            font_color="white",
            legend=dict(orientation="h", yanchor="bottom", y=-0.15, font=dict(size=9)),
            showlegend=True,
        )
        return fig

    @staticmethod
    def create_tier_badges(engines: Dict) -> html.Div:
        tier_colors = {
            "high": "success",
            "moderate": "info",
            "low": "warning",
            "very_low": "danger",
            "unknown": "secondary",
        }

        badges = []
        for engine, vals in engines.items():
            tier = vals.get("tier", "unknown")
            name = engine.replace("_", " ").title()
            badges.append(
                dbc.Badge(
                    f"{name}: {tier}",
                    color=tier_colors.get(tier, "secondary"),
                    className="me-2 mb-1",
                    style={"fontSize": "0.75rem"},
                )
            )

        return html.Div(badges, className="d-flex flex-wrap justify-content-center mt-2")

    @staticmethod
    def create_panel(data: Optional[Dict], symbol: str) -> dbc.Card:
        if not data:
            return dbc.Card(dbc.CardBody([
                html.H6("ML Calibration", className="card-title"),
                html.P("No calibration data available", className="text-muted small"),
            ]), className="h-100")

        if data.get("_unavailable"):
            return dbc.Card(dbc.CardBody([
                html.H6("ML Calibration", className="card-title"),
                html.P(data.get("_message", "Model not loaded"), className="text-warning small"),
            ]), className="h-100")

        engines = data.get("engines", {})
        regime = data.get("regime", "unknown")
        backend = data.get("backend", "unknown")

        return dbc.Card([
            dbc.CardHeader([
                html.H6([
                    "ML Calibration ",
                    dbc.Badge(f"Regime: {regime}", color="warning", className="ms-2"),
                    dbc.Badge(backend, color="info", className="ms-2", style={"fontSize": "0.65rem"}),
                ], className="mb-0"),
            ]),
            dbc.CardBody([
                dbc.Row([
                    dbc.Col(dcc.Graph(
                        figure=MLCalibrationComponent.create_comparison_chart(engines),
                        config={"displayModeBar": False},
                    ), md=6),
                    dbc.Col(dcc.Graph(
                        figure=MLCalibrationComponent.create_radar_chart(engines),
                        config={"displayModeBar": False},
                    ), md=6),
                ]),
                MLCalibrationComponent.create_tier_badges(engines),
            ]),
        ], className="h-100")
