"""Correlation & Portfolio Analytics Component"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class CorrelationMatrixComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/correlation/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching correlation: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data:
            return dbc.Card([
                dbc.CardHeader(html.H5("Correlation & Beta", className="mb-0")),
                dbc.CardBody(html.P("No correlation data available", className="text-muted"))
            ])

        correlations = data.get("correlations", [])
        beta_spy = data.get("beta_spy")
        beta_qqq = data.get("beta_qqq")
        div_score = data.get("diversification_score")
        highest = data.get("highest_correlation")
        lowest = data.get("lowest_correlation")
        rolling = data.get("rolling_correlation_spy", [])

        # Badges
        badges = []
        if beta_spy is not None:
            color = "danger" if beta_spy > 1.3 else ("success" if beta_spy < 0.8 else "info")
            badges.append(dbc.Badge(f"Beta (SPY): {beta_spy:.2f}", color=color, className="me-2 fs-6"))
        if beta_qqq is not None:
            badges.append(dbc.Badge(f"Beta (QQQ): {beta_qqq:.2f}", color="info", className="me-2"))
        if div_score is not None:
            color = "success" if div_score > 60 else ("warning" if div_score > 30 else "danger")
            badges.append(dbc.Badge(f"Diversification: {div_score:.0f}/100", color=color, className="me-2"))

        children = [html.Div(badges, className="mb-3")]

        # Correlation bars
        if correlations:
            fig = go.Figure()
            benchmarks = [c["benchmark"] for c in correlations]
            corr_values = [c["correlation"] for c in correlations]
            colors = ['#ff4444' if abs(v) > 0.8 else '#ffaa00' if abs(v) > 0.5 else '#00cc66' for v in corr_values]

            fig.add_trace(go.Bar(
                x=benchmarks, y=corr_values,
                marker_color=colors,
                text=[f"{v:.3f}" for v in corr_values],
                textposition='outside',
            ))
            fig.update_layout(
                height=200, template='plotly_dark',
                margin=dict(l=40, r=20, t=30, b=40),
                title=dict(text=f"{symbol} Correlation (90d)", font=dict(size=13)),
                yaxis=dict(range=[-1, 1], title="Pearson r"),
            )
            fig.add_hline(y=0, line_dash="dot", line_color="rgba(255,255,255,0.3)")
            children.append(dcc.Graph(figure=fig, config={'displayModeBar': False}))

        # Rolling correlation line chart
        if rolling and len(rolling) > 5:
            roll_fig = go.Figure()
            dates = [p["date"] for p in rolling]
            values = [p["correlation"] for p in rolling]
            roll_fig.add_trace(go.Scatter(
                x=dates, y=values,
                mode='lines',
                line=dict(color='#00aaff', width=2),
                name='30d Rolling Correlation',
                fill='tozeroy',
                fillcolor='rgba(0,170,255,0.1)',
            ))
            roll_fig.update_layout(
                height=180, template='plotly_dark',
                margin=dict(l=40, r=20, t=30, b=30),
                title=dict(text=f"Rolling 30d Correlation with SPY", font=dict(size=13)),
                yaxis=dict(range=[-1, 1], title="r"),
            )
            roll_fig.add_hline(y=0, line_dash="dot", line_color="rgba(255,255,255,0.2)")
            children.append(dcc.Graph(figure=roll_fig, config={'displayModeBar': False}))

        # Interpretation
        interp_parts = []
        if highest:
            h_corr = highest.get("correlation", 0)
            h_name = highest.get("benchmark", "?")
            level = "highly" if abs(h_corr) > 0.8 else ("moderately" if abs(h_corr) > 0.5 else "weakly")
            interp_parts.append(f"{symbol} is {level} correlated with {h_name} (r={h_corr:.3f}).")
        if lowest:
            l_corr = lowest.get("correlation", 0)
            l_name = lowest.get("benchmark", "?")
            interp_parts.append(f"Lowest correlation with {l_name} (r={l_corr:.3f}).")
        if interp_parts:
            children.append(html.P(
                " ".join(interp_parts),
                className="small text-muted mt-2"
            ))

        return dbc.Card([
            dbc.CardHeader(html.H5("Correlation & Beta", className="mb-0")),
            dbc.CardBody(children)
        ])
