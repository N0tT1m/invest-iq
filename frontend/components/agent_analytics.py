"""Analytics Dashboard — visualize analysis history and agent trade metrics."""
import requests
import dash_bootstrap_components as dbc
import plotly.graph_objects as go
from dash import dcc, html
from concurrent.futures import ThreadPoolExecutor

from components.config import API_BASE, get_headers, API_TIMEOUT

CHART_LAYOUT = dict(
    template="plotly_dark",
    paper_bgcolor="rgba(0,0,0,0)",
    plot_bgcolor="rgba(0,0,0,0)",
)


def _fetch(path, params=None):
    try:
        resp = requests.get(
            f"{API_BASE}{path}",
            params=params,
            headers=get_headers(),
            timeout=API_TIMEOUT,
        )
        data = resp.json()
        return data.get("data") if data.get("success") else None
    except Exception as e:
        print(f"Error fetching {path}: {e}")
        return None


# Signal label colors
SIGNAL_COLORS = {
    "StrongBuy": "#198754",
    "Buy": "#20c997",
    "Neutral": "#6c757d",
    "Sell": "#fd7e14",
    "StrongSell": "#dc3545",
}

TIER_COLORS = {
    "HIGH": "#198754",
    "MODERATE": "#ffc107",
    "LOW": "#dc3545",
    "UNKNOWN": "#6c757d",
}


class AgentAnalyticsComponent:
    @staticmethod
    def create_panel():
        """Build the analytics dashboard from analysis_features data."""
        # Fetch all analytics data in parallel (6 requests)
        with ThreadPoolExecutor(max_workers=6) as executor:
            futures = {
                'summary': executor.submit(_fetch, "/api/agent/analytics/analysis-summary"),
                'signal_dist': executor.submit(_fetch, "/api/agent/analytics/signal-distribution"),
                'regime_dist': executor.submit(_fetch, "/api/agent/analytics/regime-distribution"),
                'conviction_dist': executor.submit(_fetch, "/api/agent/analytics/conviction-distribution"),
                'top_symbols': executor.submit(_fetch, "/api/agent/analytics/top-symbols"),
                'history': executor.submit(_fetch, "/api/agent/analytics/analysis-history", {"days": 30}),
            }
            summary = futures['summary'].result() or {}
            signal_dist = futures['signal_dist'].result() or []
            regime_dist = futures['regime_dist'].result() or []
            conviction_dist = futures['conviction_dist'].result() or []
            top_symbols = futures['top_symbols'].result() or []
            history = futures['history'].result() or []

        total = summary.get("total_analyses", 0)
        if total == 0:
            return html.Div([
                html.Div([
                    html.H5("No Analytics Yet", className="text-muted mb-3"),
                    html.P(
                        "Analyze a stock to start building analytics data.",
                        className="text-muted mb-0",
                    ),
                ], className="text-center py-4"),
            ])

        children = []

        # --- Summary cards ---
        avg_conf = summary.get("avg_confidence", 0)
        unique = summary.get("unique_symbols", 0)
        breakdown = summary.get("signal_breakdown", [])
        bullish = sum(s["count"] for s in breakdown if s["signal"] in ("StrongBuy", "Buy"))
        bearish = sum(s["count"] for s in breakdown if s["signal"] in ("StrongSell", "Sell"))

        cards = dbc.Row([
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Total Analyses", className="text-muted mb-1"),
                    html.H4(str(total), className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Avg Confidence", className="text-muted mb-1"),
                    html.H4(f"{avg_conf:.1%}", className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Symbols Analyzed", className="text-muted mb-1"),
                    html.H4(str(unique), className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
            dbc.Col(dbc.Card([
                dbc.CardBody([
                    html.H6("Bull / Bear", className="text-muted mb-1"),
                    html.H4([
                        html.Span(str(bullish), className="text-success"),
                        " / ",
                        html.Span(str(bearish), className="text-danger"),
                    ], className="mb-0"),
                ], className="text-center py-2"),
            ]), md=3),
        ], className="mb-3")
        children.append(cards)

        # --- Analysis history timeline ---
        if history:
            dates = [h["date"] for h in reversed(history)]
            confs = [h["confidence"] for h in reversed(history)]
            symbols = [h["symbol"] for h in reversed(history)]
            signals = [h["signal"] for h in reversed(history)]
            colors = [SIGNAL_COLORS.get(s, "#6c757d") for s in signals]

            hist_fig = go.Figure(go.Scatter(
                x=dates, y=confs,
                mode="markers+lines",
                marker=dict(color=colors, size=8),
                line=dict(color="#495057", width=1),
                text=[f"{sym} ({sig})" for sym, sig in zip(symbols, signals)],
                hovertemplate="%{text}<br>Confidence: %{y:.1%}<br>%{x}<extra></extra>",
            ))
            hist_fig.update_layout(
                title="Analysis History",
                yaxis_title="Confidence",
                yaxis=dict(tickformat=".0%"),
                margin=dict(l=50, r=20, t=40, b=40),
                height=280,
                **CHART_LAYOUT,
            )
            children.append(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=hist_fig, config={"displayModeBar": False}))
            ], className="mb-3"))

        # --- Signal distribution + Conviction distribution side by side ---
        row1 = []
        if signal_dist:
            sigs = [s["signal"] for s in signal_dist]
            counts = [s["count"] for s in signal_dist]
            colors = [SIGNAL_COLORS.get(s, "#6c757d") for s in sigs]
            sig_fig = go.Figure(go.Bar(
                x=sigs, y=counts,
                marker_color=colors,
                text=counts,
                textposition="auto",
            ))
            sig_fig.update_layout(
                title="Signal Distribution",
                yaxis_title="Count",
                margin=dict(l=50, r=20, t=40, b=40),
                height=280,
                **CHART_LAYOUT,
            )
            row1.append(dbc.Col(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=sig_fig, config={"displayModeBar": False}))
            ]), md=6))

        if conviction_dist:
            tiers = [c["conviction_tier"] for c in conviction_dist]
            counts = [c["count"] for c in conviction_dist]
            colors = [TIER_COLORS.get(t, "#6c757d") for t in tiers]
            conv_fig = go.Figure(go.Bar(
                x=tiers, y=counts,
                marker_color=colors,
                text=counts,
                textposition="auto",
            ))
            conv_fig.update_layout(
                title="Conviction Distribution",
                yaxis_title="Count",
                margin=dict(l=50, r=20, t=40, b=40),
                height=280,
                **CHART_LAYOUT,
            )
            row1.append(dbc.Col(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=conv_fig, config={"displayModeBar": False}))
            ]), md=6))

        if row1:
            children.append(dbc.Row(row1, className="mb-3"))

        # --- Regime distribution + Top symbols side by side ---
        row2 = []
        if regime_dist:
            regimes = [r["regime"] for r in regime_dist]
            counts = [r["count"] for r in regime_dist]
            avg_confs = [r["avg_confidence"] for r in regime_dist]
            regime_fig = go.Figure(go.Bar(
                y=regimes, x=counts, orientation="h",
                marker_color="#0d6efd",
                text=[f"{c} (avg {a:.0%})" for c, a in zip(counts, avg_confs)],
                textposition="auto",
            ))
            regime_fig.update_layout(
                title="Analyses by Market Regime",
                xaxis_title="Count",
                margin=dict(l=120, r=20, t=40, b=40),
                height=280,
                **CHART_LAYOUT,
            )
            row2.append(dbc.Col(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=regime_fig, config={"displayModeBar": False}))
            ]), md=6))

        if top_symbols:
            syms = [s["symbol"] for s in top_symbols[:10]]
            counts = [s["count"] for s in top_symbols[:10]]
            avg_confs = [s["avg_confidence"] for s in top_symbols[:10]]
            sym_fig = go.Figure(go.Bar(
                y=list(reversed(syms)),
                x=list(reversed(counts)),
                orientation="h",
                marker_color="#6610f2",
                text=[f"{c} (avg {a:.0%})" for c, a in zip(reversed(counts), reversed(avg_confs))],
                textposition="auto",
            ))
            sym_fig.update_layout(
                title="Most Analyzed Symbols",
                xaxis_title="Count",
                margin=dict(l=80, r=20, t=40, b=40),
                height=280,
                **CHART_LAYOUT,
            )
            row2.append(dbc.Col(dbc.Card([
                dbc.CardBody(dcc.Graph(figure=sym_fig, config={"displayModeBar": False}))
            ]), md=6))

        if row2:
            children.append(dbc.Row(row2, className="mb-3"))

        # --- Agent trade metrics (if the agent has run) ---
        agent_summary = _fetch("/api/agent/analytics/summary") or {}
        agent_trades = agent_summary.get("total_trades", 0)
        if agent_trades > 0:
            children.append(html.Hr(className="my-3"))
            children.append(html.H5("Agent Trade Performance", className="mb-3"))

            total_pnl = agent_summary.get("total_pnl", 0)
            win_rate = agent_summary.get("win_rate", 0)
            pnl_color = "success" if total_pnl >= 0 else "danger"

            agent_cards = dbc.Row([
                dbc.Col(dbc.Card([
                    dbc.CardBody([
                        html.H6("Agent P&L", className="text-muted mb-1"),
                        html.H4(f"${total_pnl:,.2f}", className=f"text-{pnl_color} mb-0"),
                    ], className="text-center py-2"),
                ]), md=4),
                dbc.Col(dbc.Card([
                    dbc.CardBody([
                        html.H6("Agent Win Rate", className="text-muted mb-1"),
                        html.H4(f"{win_rate * 100:.1f}%", className="mb-0"),
                    ], className="text-center py-2"),
                ]), md=4),
                dbc.Col(dbc.Card([
                    dbc.CardBody([
                        html.H6("Agent Trades", className="text-muted mb-1"),
                        html.H4(str(agent_trades), className="mb-0"),
                    ], className="text-center py-2"),
                ]), md=4),
            ], className="mb-3")
            children.append(agent_cards)

            # P&L by symbol from agent trades
            pnl_data = _fetch("/api/agent/analytics/pnl-by-symbol") or []
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
                    title="Agent P&L by Symbol",
                    yaxis_title="Total P&L ($)",
                    margin=dict(l=60, r=20, t=40, b=40),
                    height=280,
                    **CHART_LAYOUT,
                )
                children.append(dbc.Card([
                    dbc.CardBody(dcc.Graph(figure=pnl_fig, config={"displayModeBar": False}))
                ], className="mb-3"))

        return html.Div(children)
