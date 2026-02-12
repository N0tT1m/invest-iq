"""Agent Analytics Dashboard — visualize trading agent performance metrics."""
import requests
import dash_bootstrap_components as dbc
import plotly.graph_objects as go
from dash import dcc, html

from components.config import API_BASE, get_headers, API_TIMEOUT


class AgentAnalyticsComponent:
    @staticmethod
    def fetch_analytics_summary():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/summary",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", {}) if data.get("success") else {}
        except Exception as e:
            print(f"Error fetching analytics summary: {e}")
            return {}

    @staticmethod
    def fetch_daily_snapshots(days=30):
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/daily-snapshots",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching daily snapshots: {e}")
            return []

    @staticmethod
    def fetch_win_rate_by_regime():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/win-rate-by-regime",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching win rate by regime: {e}")
            return []

    @staticmethod
    def fetch_win_rate_by_conviction():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/win-rate-by-conviction",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching win rate by conviction: {e}")
            return []

    @staticmethod
    def fetch_pnl_by_symbol():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/pnl-by-symbol",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching P&L by symbol: {e}")
            return []

    @staticmethod
    def fetch_confidence_calibration():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/confidence-calibration",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching confidence calibration: {e}")
            return []

    @staticmethod
    def fetch_supplementary_outcomes():
        try:
            resp = requests.get(
                f"{API_BASE}/api/agent/analytics/supplementary-outcomes",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching supplementary outcomes: {e}")
            return []

    @staticmethod
    def create_panel():
        """Build the full analytics dashboard."""
        summary = AgentAnalyticsComponent.fetch_analytics_summary()
        snapshots = AgentAnalyticsComponent.fetch_daily_snapshots(30)
        regime_data = AgentAnalyticsComponent.fetch_win_rate_by_regime()
        conviction_data = AgentAnalyticsComponent.fetch_win_rate_by_conviction()
        pnl_data = AgentAnalyticsComponent.fetch_pnl_by_symbol()
        calibration_data = AgentAnalyticsComponent.fetch_confidence_calibration()
        supp_data = AgentAnalyticsComponent.fetch_supplementary_outcomes()

        # Check if we have any data at all
        total_trades = summary.get("total_trades", 0)
        if total_trades == 0 and not snapshots:
            return html.Div([
                html.P(
                    "No analytics data yet. The trading agent will populate metrics as it runs.",
                    className="text-muted text-center py-3",
                ),
            ])

        children = []

        # Summary cards
        total_pnl = summary.get("total_pnl", 0)
        win_rate = summary.get("win_rate", 0)
        ml_rate = summary.get("ml_gate_approval_rate", 0)
        pnl_color = "success" if total_pnl >= 0 else "danger"

        cards = dbc.Row([
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Total P&L", className="text-muted mb-1"),
                    html.H4(
                        f"${total_pnl:,.2f}",
                        className=f"text-{pnl_color} mb-0",
                    ),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Win Rate", className="text-muted mb-1"),
                    html.H4(f"{win_rate * 100:.1f}%", className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Total Trades", className="text-muted mb-1"),
                    html.H4(str(total_trades), className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("ML Gate Rate", className="text-muted mb-1"),
                    html.H4(f"{ml_rate * 100:.1f}%", className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
        ], className="mb-3")
        children.append(cards)

        # Signal funnel from daily snapshots
        if snapshots:
            dates = [s["date"] for s in reversed(snapshots)]
            gen = [s.get("signals_generated", 0) for s in reversed(snapshots)]
            filt = [s.get("signals_filtered", 0) for s in reversed(snapshots)]
            approved = [s.get("signals_ml_approved", 0) for s in reversed(snapshots)]
            proposed = [s.get("trades_proposed", 0) for s in reversed(snapshots)]

            funnel_fig = go.Figure()
            funnel_fig.add_trace(go.Bar(name="Generated", x=dates, y=gen, marker_color="#6c757d"))
            funnel_fig.add_trace(go.Bar(name="Filtered", x=dates, y=filt, marker_color="#0d6efd"))
            funnel_fig.add_trace(go.Bar(name="ML Approved", x=dates, y=approved, marker_color="#198754"))
            funnel_fig.add_trace(go.Bar(name="Proposed", x=dates, y=proposed, marker_color="#ffc107"))
            funnel_fig.update_layout(
                title="Signal Funnel (Daily)",
                barmode="group",
                margin=dict(l=40, r=20, t=40, b=40),
                height=280,
                legend=dict(orientation="h", yanchor="bottom", y=1.02, xanchor="right", x=1),
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            children.append(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=funnel_fig, config={"displayModeBar": False}))
            ], className="mb-3"))

        # Win rate by regime + conviction side by side
        regime_conviction_row = []
        if regime_data:
            regimes = [r["regime"] for r in regime_data]
            wr = [r["win_rate"] * 100 for r in regime_data]
            totals = [r["total"] for r in regime_data]
            regime_fig = go.Figure(go.Bar(
                y=regimes, x=wr, orientation="h",
                marker_color="#0d6efd",
                text=[f"{w:.0f}% (n={t})" for w, t in zip(wr, totals)],
                textposition="auto",
            ))
            regime_fig.update_layout(
                title="Win Rate by Regime",
                xaxis_title="Win Rate %",
                margin=dict(l=120, r=20, t=40, b=40),
                height=280,
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            regime_conviction_row.append(
                dbc.Col(dbc.Card([
                    dbc.CardBody(dcc.Graph(figure=regime_fig, config={"displayModeBar": False}))
                ]), md=6)
            )

        if conviction_data:
            tiers = [c["conviction_tier"] for c in conviction_data]
            wr = [c["win_rate"] * 100 for c in conviction_data]
            totals = [c["total"] for c in conviction_data]
            tier_colors = {"HIGH": "#198754", "MODERATE": "#ffc107", "LOW": "#dc3545", "UNKNOWN": "#6c757d"}
            colors = [tier_colors.get(t, "#6c757d") for t in tiers]
            conv_fig = go.Figure(go.Bar(
                y=tiers, x=wr, orientation="h",
                marker_color=colors,
                text=[f"{w:.0f}% (n={t})" for w, t in zip(wr, totals)],
                textposition="auto",
            ))
            conv_fig.update_layout(
                title="Win Rate by Conviction",
                xaxis_title="Win Rate %",
                margin=dict(l=100, r=20, t=40, b=40),
                height=280,
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            regime_conviction_row.append(
                dbc.Col(dbc.Card([
                    dbc.CardBody(dcc.Graph(figure=conv_fig, config={"displayModeBar": False}))
                ]), md=6)
            )

        if regime_conviction_row:
            children.append(dbc.Row(regime_conviction_row, className="mb-3"))

        # P&L by symbol
        if pnl_data:
            symbols = [p["symbol"] for p in pnl_data]
            pnls = [p["total_pnl"] for p in pnl_data]
            colors = ["#198754" if p >= 0 else "#dc3545" for p in pnls]
            pnl_fig = go.Figure(go.Bar(
                x=symbols, y=pnls,
                marker_color=colors,
                text=[f"${p:,.0f}" for p in pnls],
                textposition="auto",
            ))
            pnl_fig.update_layout(
                title="P&L by Symbol",
                yaxis_title="Total P&L ($)",
                margin=dict(l=60, r=20, t=40, b=40),
                height=280,
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            children.append(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=pnl_fig, config={"displayModeBar": False}))
            ], className="mb-3"))

        # Confidence calibration + supplementary outcomes side by side
        bottom_row = []
        if calibration_data:
            buckets = [c["bucket"] for c in calibration_data]
            wr = [c["win_rate"] * 100 for c in calibration_data]
            totals = [c["total"] for c in calibration_data]
            cal_fig = go.Figure(go.Bar(
                x=buckets, y=wr,
                marker_color="#0d6efd",
                text=[f"{w:.0f}% (n={t})" for w, t in zip(wr, totals)],
                textposition="auto",
            ))
            cal_fig.update_layout(
                title="Confidence Calibration",
                xaxis_title="Confidence Bucket",
                yaxis_title="Actual Win Rate %",
                margin=dict(l=60, r=20, t=40, b=40),
                height=280,
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            bottom_row.append(
                dbc.Col(dbc.Card([
                    dbc.CardBody(dcc.Graph(figure=cal_fig, config={"displayModeBar": False}))
                ]), md=6)
            )

        if supp_data:
            adj_types = [s["adjustment_type"] for s in supp_data]
            wr = [s["win_rate"] * 100 for s in supp_data]
            totals = [s["total"] for s in supp_data]
            supp_fig = go.Figure(go.Bar(
                x=adj_types, y=wr,
                marker_color="#6610f2",
                text=[f"{w:.0f}% (n={t})" for w, t in zip(wr, totals)],
                textposition="auto",
            ))
            supp_fig.update_layout(
                title="Supplementary Signal Outcomes",
                xaxis_title="Signal Type",
                yaxis_title="Win Rate %",
                margin=dict(l=60, r=20, t=40, b=40),
                height=280,
                template="plotly_dark",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
            )
            bottom_row.append(
                dbc.Col(dbc.Card([
                    dbc.CardBody(dcc.Graph(figure=supp_fig, config={"displayModeBar": False}))
                ]), md=6)
            )

        if bottom_row:
            children.append(dbc.Row(bottom_row, className="mb-3"))

        return html.Div(children)
