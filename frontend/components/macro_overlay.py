"""Macro Economic Overlay Component (ETF-derived)"""
import requests
import dash_bootstrap_components as dbc
from dash import html

from components.config import API_BASE, get_headers, API_TIMEOUT


class MacroOverlayComponent:
    @staticmethod
    def fetch_indicators():
        try:
            response = requests.get(
                f"{API_BASE}/api/macro/indicators",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching macro indicators: {e}")
            return None

    @staticmethod
    def fetch_sensitivity(symbol):
        try:
            response = requests.get(
                f"{API_BASE}/api/macro/sensitivity/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching macro sensitivity: {e}")
            return None

    @staticmethod
    def create_card(indicators, sensitivity, symbol):
        children = []

        # If indicators not available, show setup message
        if not indicators or not indicators.get("available"):
            msg = indicators.get("message", "Macro data not available") if indicators else "Macro data unavailable"
            return dbc.Card([
                dbc.CardHeader(html.H5("Macro Overlay", className="mb-0")),
                dbc.CardBody([
                    html.P(msg, className="text-muted"),
                    html.Hr(),
                    html.Small("Market regime and macro indicators derived from ETF data.", className="text-muted"),
                ])
            ])

        # Market regime badge at top
        regime = indicators.get("market_regime", "Unknown")
        regime_colors = {"Risk-On": "success", "Risk-Off": "danger", "Transitioning": "warning"}
        regime_badge = dbc.Badge(
            f"Market Regime: {regime}",
            color=regime_colors.get(regime, "secondary"),
            className="fs-6 mb-3"
        )
        children.append(html.Div([regime_badge]))

        # Sensitivity badges
        if sensitivity:
            badges = []
            ir_sens = sensitivity.get("interest_rate_sensitivity")
            beta = sensitivity.get("market_beta")
            cycle = sensitivity.get("cycle_phase")

            if ir_sens is not None:
                color = "danger" if ir_sens > 60 else ("warning" if ir_sens > 30 else "success")
                badges.append(dbc.Badge(f"Rate Sensitivity: {ir_sens:.0f}/100", color=color, className="me-2"))
            if beta is not None:
                badges.append(dbc.Badge(f"Market Beta: {beta:.2f}", color="info", className="me-2"))
            if cycle:
                cycle_colors = {"Expansion": "success", "Contraction": "danger", "Transition": "warning"}
                badges.append(dbc.Badge(f"Cycle: {cycle}", color=cycle_colors.get(cycle, "secondary"), className="me-2"))

            if badges:
                children.append(html.Div(badges, className="mb-3"))

        # Indicator table with trend arrows and interpretation
        indicator_list = indicators.get("indicators", [])
        if indicator_list:
            rows = []
            for ind in indicator_list:
                val = ind.get("value")
                val_str = f"{val:.1f}{ind.get('unit', '')}" if val is not None else "N/A"
                trend = ind.get("trend")
                interp = ind.get("interpretation", "")

                # Trend arrow
                if trend == "up":
                    trend_cell = html.Td(
                        html.Span("\u25b2", style={"color": "#00cc66", "fontSize": "14px"}),
                    )
                elif trend == "down":
                    trend_cell = html.Td(
                        html.Span("\u25bc", style={"color": "#ff4444", "fontSize": "14px"}),
                    )
                else:
                    trend_cell = html.Td("\u2014", className="text-muted")

                rows.append(html.Tr([
                    html.Td(ind["name"], className="small"),
                    html.Td(val_str, className="fw-bold"),
                    trend_cell,
                    html.Td(html.Small(interp, className="text-muted")),
                ]))

            table = dbc.Table([
                html.Thead(html.Tr([
                    html.Th("Indicator"), html.Th("Value"), html.Th(""), html.Th("Interpretation")
                ])),
                html.Tbody(rows),
            ], bordered=True, dark=True, hover=True, size="sm")
            children.append(table)

        if indicators.get("message"):
            children.append(html.Small(indicators["message"], className="text-muted"))

        return dbc.Card([
            dbc.CardHeader(html.H5("Macro Overlay", className="mb-0")),
            dbc.CardBody(children if children else [html.P("No data", className="text-muted")])
        ])
