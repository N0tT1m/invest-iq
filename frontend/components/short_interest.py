"""Short Interest & Squeeze Risk Component"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class ShortInterestComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/short-interest/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching short interest: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data or not data.get("squeeze_risk_score"):
            msg = data.get("message", "No data available") if data else "No data available"
            return dbc.Card([
                dbc.CardHeader(html.H5("Short Squeeze Risk", className="mb-0")),
                dbc.CardBody(html.P(msg, className="text-muted"))
            ])

        score = data.get("squeeze_risk_score", 0)
        level = data.get("squeeze_risk_level", "Low")
        vol_spike = data.get("volume_spike")
        vol_trend = data.get("volume_trend")
        momentum = data.get("price_momentum")
        vol_pctile = data.get("volatility_percentile")
        bb_squeeze = data.get("bb_squeeze")
        bb_width = data.get("bb_width")
        rsi_val = data.get("rsi")
        components = data.get("components", [])
        interpretation = data.get("interpretation")

        level_color = {"High": "danger", "Moderate": "warning", "Low": "success"}.get(level, "secondary")

        # Squeeze risk gauge
        gauge = go.Figure(go.Indicator(
            mode="gauge+number",
            value=score,
            title={'text': "Squeeze Risk", 'font': {'size': 14}},
            gauge={
                'axis': {'range': [0, 100]},
                'bar': {'color': '#ff6600' if score > 40 else '#00cc66'},
                'steps': [
                    {'range': [0, 33], 'color': '#1a472a'},
                    {'range': [33, 66], 'color': '#5a4a2a'},
                    {'range': [66, 100], 'color': '#6a2a2a'},
                ],
            },
        ))
        gauge.update_layout(
            height=180, template='plotly_dark',
            margin=dict(l=30, r=30, t=40, b=10),
        )

        # Top badges
        badges = [dbc.Badge(f"Risk: {level}", color=level_color, className="me-2 fs-6")]
        if bb_squeeze is not None:
            sq_color = "danger" if bb_squeeze else "secondary"
            sq_text = "BB Squeeze: ACTIVE" if bb_squeeze else "BB Squeeze: Inactive"
            badges.append(dbc.Badge(sq_text, color=sq_color, className="me-2"))
        if vol_pctile is not None:
            badges.append(dbc.Badge(f"Vol %ile: {vol_pctile:.0f}%", color="info", className="me-2"))
        if rsi_val is not None:
            rsi_color = "danger" if rsi_val > 70 else ("success" if rsi_val < 30 else "secondary")
            badges.append(dbc.Badge(f"RSI: {rsi_val:.1f}", color=rsi_color, className="me-2"))

        children = [
            html.Div(badges, className="mb-2"),
            dcc.Graph(figure=gauge, config={'displayModeBar': False}),
        ]

        # Score breakdown chart
        if components:
            comp_fig = go.Figure()
            names = [c["name"] for c in components]
            scores = [c["score"] for c in components]
            max_scores = [c["max_score"] for c in components]

            # Background (max possible)
            comp_fig.add_trace(go.Bar(
                x=max_scores, y=names, orientation='h',
                marker_color='rgba(255,255,255,0.1)',
                name='Max', showlegend=False,
                hoverinfo='skip',
            ))
            # Actual scores
            colors = ['#ff6600' if s/m > 0.6 else '#ffaa00' if s/m > 0.3 else '#00cc66'
                       for s, m in zip(scores, max_scores)]
            comp_fig.add_trace(go.Bar(
                x=scores, y=names, orientation='h',
                marker_color=colors,
                name='Score',
                text=[f"{s:.1f}/{m:.0f}" for s, m in zip(scores, max_scores)],
                textposition='outside',
            ))
            comp_fig.update_layout(
                barmode='overlay', height=200, template='plotly_dark',
                margin=dict(l=100, r=50, t=25, b=10),
                showlegend=False,
                xaxis=dict(range=[0, 25], title="Score"),
                title=dict(text="Score Breakdown", font=dict(size=13)),
            )
            children.append(dcc.Graph(figure=comp_fig, config={'displayModeBar': False}))

        # Additional metric badges
        extra_badges = []
        if vol_spike is not None:
            v_color = "warning" if vol_spike > 1.5 else "secondary"
            extra_badges.append(dbc.Badge(f"Vol Spike: {vol_spike:.1f}x", color=v_color, className="me-2"))
        if vol_trend is not None:
            vt_color = "warning" if vol_trend > 1.2 else "secondary"
            extra_badges.append(dbc.Badge(f"Vol Trend: {vol_trend:.2f}x", color=vt_color, className="me-2"))
        if momentum is not None:
            m_color = "success" if momentum > 0 else "danger"
            extra_badges.append(dbc.Badge(f"Momentum: {momentum:+.1f}%", color=m_color, className="me-2"))
        if bb_width is not None:
            extra_badges.append(dbc.Badge(f"BB Width: {bb_width:.3f}", color="secondary", className="me-2"))
        if extra_badges:
            children.append(html.Div(extra_badges, className="mb-2"))

        # Interpretation text
        if interpretation:
            children.append(html.P(interpretation, className="small text-muted mt-2"))

        return dbc.Card([
            dbc.CardHeader(html.H5("Short Squeeze Risk", className="mb-0")),
            dbc.CardBody(children)
        ])
