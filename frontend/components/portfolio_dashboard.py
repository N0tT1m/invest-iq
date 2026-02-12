"""Portfolio Dashboard Component - positions, allocation, P&L, bank accounts, transfers."""
import json
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT

try:
    import requests
except ImportError:
    pass

DEFAULT_BANK_ACCOUNTS = [
    {"id": "pnc", "name": "PNC Bank", "lastFour": "4821", "balance": 0, "color": "#F58220"},
    {"id": "cap1", "name": "Capital One", "lastFour": "7135", "balance": 0, "color": "#D03027"},
]


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
    def create_dashboard(account, positions, orders, bank_accounts=None, transfer_history=None):
        """Build the full portfolio dashboard card.

        Args:
            account: Alpaca account dict (or None if not configured)
            positions: list of position dicts
            orders: list of order dicts
            bank_accounts: list of bank account dicts (from dcc.Store)
            transfer_history: list of transfer record dicts (from dcc.Store)
        """
        if bank_accounts is None:
            bank_accounts = DEFAULT_BANK_ACCOUNTS
        if transfer_history is None:
            transfer_history = []

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
        equity = float(account.get("equity", 0))
        last_equity = float(account.get("last_equity", 0) or 0)

        # P&L totals
        total_unrealized_pl = sum(float(p.get("unrealized_pl", 0)) for p in positions)
        total_cost_basis = sum(float(p.get("cost_basis", 0)) for p in positions)
        total_pl_pct = (total_unrealized_pl / total_cost_basis * 100) if total_cost_basis > 0 else 0
        day_change = equity - last_equity if last_equity > 0 else 0
        day_change_pct = (day_change / last_equity * 100) if last_equity > 0 else 0

        pl_color = "text-success" if total_unrealized_pl >= 0 else "text-danger"
        pl_sign = "+" if total_unrealized_pl >= 0 else ""
        day_color = "text-success" if day_change >= 0 else "text-danger"
        day_sign = "+" if day_change >= 0 else ""

        # --- Row 1: Account Summary ---
        account_row = dbc.Row([
            dbc.Col([
                html.Small("Equity", className="text-muted d-block"),
                html.H4(f"${equity:,.2f}", className="text-info mb-0"),
            ], md=2, className="text-center"),
            dbc.Col([
                html.Small("Cash", className="text-muted d-block"),
                html.H4(f"${cash:,.2f}", className="text-warning mb-0"),
            ], md=2, className="text-center"),
            dbc.Col([
                html.Small("Buying Power", className="text-muted d-block"),
                html.H4(f"${buying_power:,.2f}", className="text-success mb-0"),
            ], md=2, className="text-center"),
            dbc.Col([
                html.Small("Unrealized P&L", className="text-muted d-block"),
                html.H4(f"{pl_sign}${total_unrealized_pl:,.2f}", className=f"{pl_color} mb-0"),
                html.Small(f"{pl_sign}{total_pl_pct:.2f}%", className=pl_color),
            ], md=3, className="text-center"),
            dbc.Col([
                html.Small("Day Change", className="text-muted d-block"),
                html.H4(f"{day_sign}${day_change:,.2f}", className=f"{day_color} mb-0"),
                html.Small(f"{day_sign}{day_change_pct:.2f}%", className=day_color),
            ], md=3, className="text-center"),
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

        # --- P&L Bar Chart ---
        pl_chart_section = PortfolioDashboardComponent._create_pl_bar_chart(positions)

        # --- Bank Accounts ---
        bank_section = PortfolioDashboardComponent._create_bank_accounts_section(bank_accounts)

        # --- Transfer Form ---
        transfer_section = PortfolioDashboardComponent._create_transfer_section(bank_accounts, transfer_history)

        # --- Recent Orders ---
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
                pl_chart_section,
                html.Hr(),
                bank_section,
                html.Hr(),
                transfer_section,
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

    @staticmethod
    def _create_pl_bar_chart(positions):
        """Create a horizontal bar chart showing P&L per position."""
        if not positions:
            return html.Div()

        sorted_positions = sorted(
            positions,
            key=lambda p: float(p.get("unrealized_pl", 0)),
            reverse=True,
        )

        symbols = [p.get("symbol", "?") for p in sorted_positions]
        pnl_values = [float(p.get("unrealized_pl", 0)) for p in sorted_positions]
        colors = ["#00CC96" if v >= 0 else "#EF553B" for v in pnl_values]

        fig = go.Figure(data=[go.Bar(
            y=symbols,
            x=pnl_values,
            orientation="h",
            marker=dict(color=colors),
            text=[f"${v:+,.2f}" for v in pnl_values],
            textposition="auto",
            textfont=dict(size=11),
        )])
        fig.update_layout(
            template="plotly_dark",
            height=max(200, len(symbols) * 40),
            margin=dict(l=60, r=20, t=10, b=10),
            xaxis=dict(title="P&L ($)", gridcolor="rgba(255,255,255,0.1)"),
            yaxis=dict(autorange="reversed"),
            plot_bgcolor="rgba(0,0,0,0)",
            paper_bgcolor="rgba(0,0,0,0)",
        )
        fig.add_vline(x=0, line_dash="dash", line_color="rgba(255,255,255,0.3)")

        return html.Div([
            html.H6("P&L by Position", className="mb-2"),
            dcc.Graph(figure=fig, config={"displayModeBar": False}),
        ])

    @staticmethod
    def _create_bank_accounts_section(bank_accounts):
        """Create bank account cards for PNC and Capital One."""
        cards = []
        for bank in bank_accounts:
            balance = bank.get("balance", 0)
            color = bank.get("color", "#888")
            cards.append(
                dbc.Col([
                    dbc.Card([
                        dbc.CardBody([
                            html.Div([
                                html.I(className="fas fa-university me-2", style={"color": color, "fontSize": "1.3rem"}),
                                html.Div([
                                    html.Strong(bank.get("name", "Bank"), className="d-block"),
                                    html.Small(f"****{bank.get('lastFour', '0000')}", className="text-muted"),
                                ]),
                            ], className="d-flex align-items-center mb-2"),
                            html.H4(f"${balance:,.2f}", className="mb-0"),
                            html.Small("Transferred total", className="text-muted"),
                        ]),
                    ], style={"borderLeft": f"4px solid {color}"}),
                ], md=6),
            )

        return html.Div([
            html.H6("Linked Bank Accounts", className="mb-2"),
            dbc.Row(cards),
        ])

    @staticmethod
    def _create_transfer_section(bank_accounts, transfer_history):
        """Create the transfer form and history table."""
        pnc = next((b for b in bank_accounts if b["id"] == "pnc"), bank_accounts[0] if bank_accounts else {})
        cap1 = next((b for b in bank_accounts if b["id"] == "cap1"), bank_accounts[1] if len(bank_accounts) > 1 else {})

        preset_buttons = []
        for label, pnc_pct in [("20/80", 20), ("40/60", 40), ("50/50", 50), ("60/40", 60), ("80/20", 80)]:
            preset_buttons.append(
                dbc.Button(
                    label,
                    id={"type": "transfer-preset-btn", "index": pnc_pct},
                    color="outline-primary",
                    size="sm",
                    className="me-1",
                )
            )

        transfer_form = dbc.Card([
            dbc.CardBody([
                html.Div([
                    html.I(className="fas fa-exchange-alt me-2 text-primary"),
                    html.H6("Transfer Funds", className="mb-0 d-inline"),
                ], className="d-flex align-items-center mb-3"),
                dbc.InputGroup([
                    dbc.InputGroupText("$"),
                    dbc.Input(
                        id="transfer-amount-input",
                        type="number",
                        placeholder="Enter amount",
                        min=0,
                        step=100,
                    ),
                ], className="mb-3"),
                html.Small("Split Ratio (PNC / Capital One)", className="text-muted d-block mb-2"),
                html.Div(preset_buttons, className="mb-2"),
                dcc.Slider(
                    id="transfer-split-slider",
                    min=0,
                    max=100,
                    step=5,
                    value=40,
                    marks={0: "0%", 25: "25%", 50: "50%", 75: "75%", 100: "100%"},
                    tooltip={"placement": "bottom", "always_visible": False},
                    className="mb-3",
                ),
                html.Div(id="transfer-preview", className="mb-3"),
                dbc.Button(
                    "Transfer",
                    id="transfer-confirm-btn",
                    color="primary",
                    className="w-100",
                    disabled=True,
                ),
            ]),
        ])

        # Transfer history
        if transfer_history:
            history_rows = []
            for t in transfer_history[:20]:
                date_str = t.get("date", "")
                if date_str and len(date_str) > 10:
                    date_str = date_str[:10]
                total = t.get("totalAmount", 0)
                splits = t.get("splits", [])
                pnc_split = next((s for s in splits if s.get("bankId") == "pnc"), None)
                cap1_split = next((s for s in splits if s.get("bankId") == "cap1"), None)

                history_rows.append(html.Tr([
                    html.Td(date_str, className="small"),
                    html.Td(f"${total:,.2f}"),
                    html.Td(f"${pnc_split['amount']:,.2f} ({pnc_split['pct']}%)" if pnc_split else "-"),
                    html.Td(f"${cap1_split['amount']:,.2f} ({cap1_split['pct']}%)" if cap1_split else "-"),
                ]))

            history_table = dbc.Table(
                [html.Thead(html.Tr([
                    html.Th("Date"), html.Th("Total"),
                    html.Th("PNC"), html.Th("Capital One"),
                ]))] + [html.Tbody(history_rows)],
                bordered=True, hover=True, responsive=True,
                className="table-dark table-sm mt-3",
            )
        else:
            history_table = html.P("No transfers yet", className="text-muted small mt-3")

        return html.Div([
            transfer_form,
            html.H6("Transfer History", className="mt-3 mb-2"),
            history_table,
        ])
