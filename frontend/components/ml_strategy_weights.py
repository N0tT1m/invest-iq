"""
ML Strategy Weights Component

Bayesian strategy weights, credible intervals, and stats.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, Optional, List
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class MLStrategyWeightsComponent:

    @staticmethod
    def fetch_data() -> Optional[Dict]:
        try:
            resp = requests.get(
                f"{API_BASE}/api/ml/strategy-weights",
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
            print(f"ML strategy weights error: {e}")
            return None

    @staticmethod
    def create_weights_bar(data: Dict) -> go.Figure:
        weights = data.get("weights", {})
        if not weights:
            fig = go.Figure()
            fig.update_layout(height=220, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        names = list(weights.keys())
        values = [weights[n] * 100 for n in names]
        display_names = [n.replace("_", " ").title() for n in names]

        colors = ["#636efa", "#00cc96", "#ef553b", "#ffa15a", "#ab63fa"]

        fig = go.Figure(go.Bar(
            x=display_names, y=values,
            marker_color=colors[:len(names)],
            text=[f"{v:.1f}%" for v in values],
            textposition="outside",
        ))
        fig.update_layout(
            title="Engine Weights",
            yaxis=dict(range=[0, max(values) * 1.3 if values else 100], title="%"),
            height=220, margin=dict(t=35, b=30, l=40, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white",
        )
        return fig

    @staticmethod
    def create_forest_plot(strategies: List[Dict]) -> go.Figure:
        """Forest plot showing win rates with credible intervals."""
        if not strategies:
            fig = go.Figure()
            fig.update_layout(height=220, paper_bgcolor="rgba(0,0,0,0)")
            return fig

        names = []
        win_rates = []
        ci_lows = []
        ci_highs = []

        for s in strategies:
            names.append(s.get("name", "").replace("_", " ").title())
            wr = s.get("win_rate", 0.5)
            win_rates.append(wr * 100)
            ci = s.get("credible_interval")
            if ci:
                ci_lows.append(ci[0] * 100)
                ci_highs.append(ci[1] * 100)
            else:
                ci_lows.append(wr * 100 - 10)
                ci_highs.append(wr * 100 + 10)

        error_low = [wr - lo for wr, lo in zip(win_rates, ci_lows)]
        error_high = [hi - wr for wr, hi in zip(win_rates, ci_highs)]

        fig = go.Figure(go.Scatter(
            x=win_rates, y=names,
            mode="markers",
            marker=dict(size=10, color="#636efa"),
            error_x=dict(
                type="data",
                symmetric=False,
                array=error_high,
                arrayminus=error_low,
                color="rgba(99,110,250,0.5)",
            ),
        ))

        # 50% reference line
        fig.add_vline(x=50, line_dash="dash", line_color="rgba(255,255,255,0.3)")

        fig.update_layout(
            title="Win Rate (95% CI)",
            xaxis=dict(range=[0, 100], title="Win Rate %"),
            height=220, margin=dict(t=35, b=30, l=100, r=10),
            paper_bgcolor="rgba(0,0,0,0)", plot_bgcolor="rgba(0,0,0,0)",
            font_color="white",
        )
        return fig

    @staticmethod
    def create_stats_table(strategies: List[Dict]) -> dbc.Table:
        if not strategies:
            return html.P("No strategy data", className="text-muted small")

        header = html.Thead(html.Tr([
            html.Th("Strategy"), html.Th("Win Rate"),
            html.Th("Samples"), html.Th("Weight"),
        ]))

        rows = []
        for s in strategies:
            wr = s.get("win_rate", 0)
            wr_color = "text-success" if wr > 0.55 else "text-danger" if wr < 0.45 else ""
            rows.append(html.Tr([
                html.Td(s.get("name", "").replace("_", " ").title()),
                html.Td(f"{wr:.1%}", className=wr_color),
                html.Td(str(s.get("total_samples", 0))),
                html.Td(f"{s.get('weight', 0):.1%}"),
            ]))

        return dbc.Table(
            [header, html.Tbody(rows)],
            bordered=True, dark=True, hover=True, size="sm",
            className="mb-0",
        )

    @staticmethod
    def create_panel(data: Optional[Dict]) -> dbc.Card:
        if not data:
            return dbc.Card(dbc.CardBody([
                html.H6("ML Strategy Weights", className="card-title"),
                html.P("No strategy data available", className="text-muted small"),
            ]), className="h-100")

        if data.get("_unavailable"):
            return dbc.Card(dbc.CardBody([
                html.H6("ML Strategy Weights", className="card-title"),
                html.P(data.get("_message", "Model not loaded"), className="text-warning small"),
            ]), className="h-100")

        strategies = data.get("strategies", [])
        backend = data.get("backend", "unknown")

        return dbc.Card([
            dbc.CardHeader([
                html.H6([
                    "ML Strategy Weights (Bayesian) ",
                    dbc.Badge(f"{len(strategies)} strategies", color="secondary", className="ms-2"),
                    dbc.Badge(backend, color="info", className="ms-2", style={"fontSize": "0.65rem"}),
                ], className="mb-0"),
            ]),
            dbc.CardBody([
                dbc.Row([
                    dbc.Col(dcc.Graph(
                        figure=MLStrategyWeightsComponent.create_weights_bar(data),
                        config={"displayModeBar": False},
                    ), md=4),
                    dbc.Col(dcc.Graph(
                        figure=MLStrategyWeightsComponent.create_forest_plot(strategies),
                        config={"displayModeBar": False},
                    ), md=4),
                    dbc.Col(
                        MLStrategyWeightsComponent.create_stats_table(strategies),
                        md=4, style={"maxHeight": "220px", "overflowY": "auto"},
                    ),
                ]),
            ]),
        ], className="h-100")
