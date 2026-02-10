"""Options Flow Component"""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class OptionsFlowComponent:
    @staticmethod
    def fetch_data(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/options/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching options: {e}")
            return None

    @staticmethod
    def create_card(data, symbol):
        if not data or not data.get("available"):
            msg = data.get("message", "Options data unavailable") if data else "No options data available"
            return dbc.Card([
                dbc.CardHeader(html.H5("Options Flow", className="mb-0")),
                dbc.CardBody(html.P(msg, className="text-muted"))
            ])

        data_source = data.get("data_source", "premium")

        # HV Proxy fallback view
        if data_source == "hv_proxy":
            hv_proxy = data.get("hv_proxy")
            hv_rank = data.get("hv_rank")
            max_dd = data.get("max_drawdown")
            beta = data.get("beta")

            children = [
                dbc.Badge("Historical Vol Proxy", color="warning", className="mb-2 me-2"),
            ]

            badges = []
            if hv_proxy is not None:
                badges.append(dbc.Badge(f"HV (20d): {hv_proxy:.1f}%", color="info", className="me-2 fs-6"))
            if hv_rank is not None:
                color = "danger" if hv_rank > 70 else ("warning" if hv_rank > 40 else "success")
                badges.append(dbc.Badge(f"HV Rank: {hv_rank:.0f}%", color=color, className="me-2 fs-6"))
            if max_dd is not None:
                badges.append(dbc.Badge(f"Max Drawdown: {max_dd:.1f}%", color="danger", className="me-2"))
            if beta is not None:
                badges.append(dbc.Badge(f"Beta: {beta:.2f}", color="info", className="me-2"))
            children.append(html.Div(badges, className="mb-3"))

            # HV Rank gauge
            if hv_rank is not None:
                gauge = go.Figure(go.Indicator(
                    mode="gauge+number",
                    value=hv_rank,
                    title={'text': "HV Rank (IV Proxy)", 'font': {'size': 14}},
                    gauge={
                        'axis': {'range': [0, 100]},
                        'bar': {'color': '#00aaff'},
                        'steps': [
                            {'range': [0, 25], 'color': '#1a472a'},
                            {'range': [25, 50], 'color': '#3a5a3a'},
                            {'range': [50, 75], 'color': '#5a4a2a'},
                            {'range': [75, 100], 'color': '#6a2a2a'},
                        ],
                    },
                    number={'suffix': '%'},
                ))
                gauge.update_layout(
                    height=180, template='plotly_dark',
                    margin=dict(l=30, r=30, t=40, b=10),
                )
                children.append(dcc.Graph(figure=gauge, config={'displayModeBar': False}))

            children.append(html.P(
                "Options chain requires a premium plan. HV Rank uses historical volatility "
                "as an IV proxy. A high rank means current vol is elevated vs. recent history.",
                className="text-muted small mt-2"
            ))

            return dbc.Card([
                dbc.CardHeader(html.H5("Options Flow", className="mb-0")),
                dbc.CardBody(children)
            ])

        # Premium: full options view
        iv_rank = data.get("iv_rank")
        pcr = data.get("put_call_ratio")
        avg_iv = data.get("avg_implied_volatility")
        max_pain = data.get("max_pain")
        unusual = data.get("unusual_activity", [])
        call_vol = data.get("total_call_volume", 0)
        put_vol = data.get("total_put_volume", 0)

        children = []

        # Top badges row
        badges = []
        if iv_rank is not None:
            color = "danger" if iv_rank > 70 else ("warning" if iv_rank > 40 else "success")
            badges.append(dbc.Badge(f"IV Rank: {iv_rank:.0f}", color=color, className="me-2 fs-6"))
        if pcr is not None:
            color = "danger" if pcr > 1.0 else "success"
            badges.append(dbc.Badge(f"P/C Ratio: {pcr:.2f}", color=color, className="me-2 fs-6"))
        if avg_iv is not None:
            badges.append(dbc.Badge(f"Avg IV: {avg_iv:.1f}%", color="info", className="me-2"))
        if max_pain is not None:
            badges.append(dbc.Badge(f"Max Pain: ${max_pain:.2f}", color="warning", className="me-2"))
        children.append(html.Div(badges, className="mb-3"))

        # IV Rank gauge
        if iv_rank is not None:
            gauge = go.Figure(go.Indicator(
                mode="gauge+number",
                value=iv_rank,
                title={'text': "IV Rank", 'font': {'size': 14}},
                gauge={
                    'axis': {'range': [0, 100]},
                    'bar': {'color': '#00aaff'},
                    'steps': [
                        {'range': [0, 25], 'color': '#1a472a'},
                        {'range': [25, 50], 'color': '#3a5a3a'},
                        {'range': [50, 75], 'color': '#5a4a2a'},
                        {'range': [75, 100], 'color': '#6a2a2a'},
                    ],
                },
                number={'suffix': '%'},
            ))
            gauge.update_layout(
                height=180, template='plotly_dark',
                margin=dict(l=30, r=30, t=40, b=10),
            )
            children.append(dcc.Graph(figure=gauge, config={'displayModeBar': False}))

        # Put/Call volume bar
        if call_vol or put_vol:
            fig = go.Figure()
            fig.add_trace(go.Bar(
                y=['Volume'], x=[call_vol], orientation='h',
                name='Calls', marker_color='#00cc66',
                text=[f"Calls: {call_vol:,}"], textposition='inside',
            ))
            fig.add_trace(go.Bar(
                y=['Volume'], x=[put_vol], orientation='h',
                name='Puts', marker_color='#ff4444',
                text=[f"Puts: {put_vol:,}"], textposition='inside',
            ))
            fig.update_layout(
                barmode='stack', height=80, template='plotly_dark',
                margin=dict(l=10, r=10, t=5, b=5),
                showlegend=False, yaxis_visible=False,
            )
            children.append(dcc.Graph(figure=fig, config={'displayModeBar': False}))

        # Unusual activity table
        if unusual:
            rows = []
            for u in unusual[:5]:
                rows.append(html.Tr([
                    html.Td(u.get("contract_type", "").upper(), className="text-success" if u.get("contract_type") == "call" else "text-danger"),
                    html.Td(f"${u.get('strike', 0):.0f}"),
                    html.Td(u.get("expiration", "")[:10]),
                    html.Td(f"{u.get('volume', 0):,}"),
                    html.Td(f"{u.get('vol_oi_ratio', 0):.1f}x"),
                ]))
            table = dbc.Table([
                html.Thead(html.Tr([
                    html.Th("Type"), html.Th("Strike"), html.Th("Exp"),
                    html.Th("Vol"), html.Th("Vol/OI"),
                ])),
                html.Tbody(rows),
            ], bordered=True, dark=True, hover=True, size="sm", className="mt-2 mb-0")
            children.append(html.H6("Unusual Activity", className="mt-3 mb-1"))
            children.append(table)

        return dbc.Card([
            dbc.CardHeader(html.H5("Options Flow", className="mb-0")),
            dbc.CardBody(children)
        ])
