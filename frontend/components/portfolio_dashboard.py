"""Portfolio Dashboard Component - full positions, allocation, and order history."""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class PortfolioDashboardComponent:
    @staticmethod
    def fetch_account():
        """Fetch Alpaca paper trading account info."""
        try:
            response = requests.get(
                f"{API_BASE}/api/broker/account",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"[portfolio-dashboard] Error fetching account: {e}")
            return None

    @staticmethod
    def fetch_positions():
        """Fetch all Alpaca broker positions."""
        try:
            response = requests.get(
                f"{API_BASE}/api/broker/positions",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"[portfolio-dashboard] Error fetching positions: {e}")
            return []

    @staticmethod
    def fetch_orders(limit=20):
        """Fetch recent orders from Alpaca broker."""
        try:
            response = requests.get(
                f"{API_BASE}/api/broker/orders",
                params={"limit": limit},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"[portfolio-dashboard] Error fetching orders: {e}")
            return []

    @staticmethod
    def create_dashboard(account, positions, orders):
        """Build the full portfolio dashboard card.

        Args:
            account: Alpaca account dict (or None if not configured)
            positions: list of position dicts
            orders: list of order dicts
        """
        if account is None:
            return dbc.Card([
                dbc.CardHeader(
                    html.Div([
                        html.H5("Portfolio Dashboard", className="mb-0 d-inline"),
                        dbc.Badge("PAPER", color="info", className="ms-2"),
                    ], className="d-flex align-items-center")
                ),
                dbc.CardBody(
                    dbc.Alert(
                        "Alpaca broker not configured. Set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables to enable paper trading.",
                        color="warning",
                    )
                ),
            ])

        # Alpaca returns all numeric fields as strings
        portfolio_value = float(account.get("portfolio_value", 0))
        buying_power = float(account.get("buying_power", 0))
        cash = float(account.get("cash", 0))

        # --- Row 1: Account Summary ---
        account_row = dbc.Row([
            dbc.Col([
                html.Small("Portfolio Value", className="text-muted d-block"),
                html.H4(f"${portfolio_value:,.2f}", className="text-info mb-0"),
            ], md=4, className="text-center"),
            dbc.Col([
                html.Small("Buying Power", className="text-muted d-block"),
                html.H4(f"${buying_power:,.2f}", className="text-success mb-0"),
            ], md=4, className="text-center"),
            dbc.Col([
                html.Small("Cash", className="text-muted d-block"),
                html.H4(f"${cash:,.2f}", className="text-warning mb-0"),
            ], md=4, className="text-center"),
        ], className="mb-3")

        # --- Row 2: Positions table + Allocation chart ---
        if positions:
            pos_rows = []
            for pos in positions:
                sym = pos.get("symbol", "?")
                qty = float(pos.get("qty", 0))
                avg_cost = float(pos.get("avg_entry_price", 0))
                current = float(pos.get("current_price", 0))
                mkt_value = float(pos.get("market_value", 0))
                pnl = float(pos.get("unrealized_pl", 0))
                pnl_pct = float(pos.get("unrealized_plpc", 0)) * 100
                change_today = float(pos.get("change_today", 0)) * 100

                pnl_color = "text-success" if pnl >= 0 else "text-danger"
                pnl_sign = "+" if pnl >= 0 else ""
                today_color = "text-success" if change_today >= 0 else "text-danger"
                today_sign = "+" if change_today >= 0 else ""

                pos_rows.append(html.Tr([
                    html.Td(html.Strong(sym)),
                    html.Td(f"{qty:g}"),
                    html.Td(f"${avg_cost:.2f}"),
                    html.Td(f"${current:.2f}"),
                    html.Td(f"${mkt_value:,.2f}"),
                    html.Td(f"{pnl_sign}${pnl:,.2f}", className=pnl_color),
                    html.Td(f"{pnl_sign}{pnl_pct:.1f}%", className=pnl_color),
                    html.Td(f"{today_sign}{change_today:.1f}%", className=today_color),
                ]))

            positions_table = dbc.Table(
                [html.Thead(html.Tr([
                    html.Th("Symbol"), html.Th("Qty"), html.Th("Avg Cost"),
                    html.Th("Current"), html.Th("Mkt Value"),
                    html.Th("P&L ($)"), html.Th("P&L (%)"), html.Th("Today"),
                ]))] + [html.Tbody(pos_rows)],
                bordered=True, hover=True, responsive=True,
                className="table-dark table-sm",
            )

            allocation_chart = PortfolioDashboardComponent._create_allocation_chart(positions)
            chart_block = dcc.Graph(figure=allocation_chart, config={"displayModeBar": False})
        else:
            positions_table = html.P("No open positions", className="text-muted text-center py-3")
            chart_block = html.P("No positions to chart", className="text-muted text-center py-3")

        positions_row = dbc.Row([
            dbc.Col([
                html.H6("Open Positions", className="mb-2"),
                positions_table,
            ], md=7),
            dbc.Col([
                html.H6("Allocation", className="mb-2"),
                chart_block,
            ], md=5),
        ])

        # --- Row 3: Recent Orders ---
        if orders:
            order_rows = []
            for order in orders[:20]:
                submitted_at = order.get("submitted_at", "")
                if submitted_at and len(submitted_at) > 16:
                    submitted_at = submitted_at[:16].replace("T", " ")

                sym = order.get("symbol", "?")
                side = order.get("side", "?")
                qty = order.get("qty", "?")
                filled_price = order.get("filled_avg_price")
                price_display = f"${float(filled_price):.2f}" if filled_price else "-"
                status = order.get("status", "unknown")

                side_color = "text-success" if side == "buy" else "text-danger"
                status_colors = {
                    "filled": "success",
                    "partially_filled": "info",
                    "new": "primary",
                    "accepted": "primary",
                    "canceled": "secondary",
                    "rejected": "danger",
                    "pending_new": "warning",
                }
                badge_color = status_colors.get(status, "secondary")

                order_id = order.get("id", "")
                cancelable = status in ("new", "accepted", "pending_new", "partially_filled")

                action_cell = html.Td("")
                if cancelable and order_id:
                    action_cell = html.Td(
                        dbc.Button(
                            "Cancel",
                            id={"type": "order-cancel-btn", "index": order_id},
                            color="warning",
                            size="sm",
                        )
                    )
                elif status == "filled" and order_id:
                    action_cell = html.Td(
                        dbc.Button(
                            "Close",
                            id={"type": "position-close-btn", "index": sym},
                            color="outline-danger",
                            size="sm",
                        )
                    )

                order_rows.append(html.Tr([
                    html.Td(submitted_at, className="small"),
                    html.Td(sym),
                    html.Td(side.upper(), className=f"{side_color} fw-bold"),
                    html.Td(str(qty)),
                    html.Td(price_display),
                    html.Td(dbc.Badge(status, color=badge_color)),
                    action_cell,
                ]))

            orders_table = dbc.Table(
                [html.Thead(html.Tr([
                    html.Th("Time"), html.Th("Symbol"), html.Th("Side"),
                    html.Th("Qty"), html.Th("Fill Price"), html.Th("Status"),
                    html.Th(""),
                ]))] + [html.Tbody(order_rows)],
                bordered=True, hover=True, responsive=True,
                className="table-dark table-sm",
            )
            orders_section = html.Div([
                html.H6("Recent Orders", className="mb-2"),
                orders_table,
            ])
        else:
            orders_section = html.P("No recent orders", className="text-muted small")

        return dbc.Card([
            dbc.CardHeader(
                html.Div([
                    html.H5("Portfolio Dashboard", className="mb-0 d-inline"),
                    dbc.Badge("PAPER", color="info", className="ms-2"),
                ], className="d-flex align-items-center")
            ),
            dbc.CardBody([
                account_row,
                html.Hr(),
                positions_row,
                html.Hr(),
                orders_section,
            ]),
        ])

    @staticmethod
    def _create_allocation_chart(positions):
        """Create a donut pie chart showing portfolio allocation by position."""
        labels = []
        values = []
        for pos in positions:
            sym = pos.get("symbol", "?")
            mkt_value = abs(float(pos.get("market_value", 0)))
            if mkt_value > 0:
                labels.append(sym)
                values.append(mkt_value)

        if not labels:
            fig = go.Figure()
            fig.add_annotation(
                text="No positions", xref="paper", yref="paper",
                x=0.5, y=0.5, showarrow=False, font=dict(size=14, color="gray"),
            )
            fig.update_layout(
                template="plotly_dark", height=280,
                xaxis=dict(visible=False), yaxis=dict(visible=False),
            )
            return fig

        fig = go.Figure(data=[go.Pie(
            labels=labels,
            values=values,
            hole=0.45,
            textinfo="label+percent",
            textfont=dict(size=11),
            marker=dict(line=dict(color="#1a1a2e", width=2)),
        )])
        fig.update_layout(
            template="plotly_dark",
            height=280,
            margin=dict(l=20, r=20, t=20, b=20),
            showlegend=False,
        )
        return fig
