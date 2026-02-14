"""Live Trading Panel Component for main dashboard integration."""
import os
import requests
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class LiveTradingComponent:
    @staticmethod
    def fetch_account():
        """Fetch Alpaca live trading account info."""
        try:
            response = requests.get(
                f"{API_BASE}/api/broker/account",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Error fetching broker account: {e}")
            return None

    @staticmethod
    def fetch_positions():
        """Fetch all Alpaca broker positions."""
        try:
            response = requests.get(
                f"{API_BASE}/api/broker/positions",
                headers=get_headers(),
                timeout=API_TIMEOUT
            )
            data = response.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching broker positions: {e}")
            return []

    @staticmethod
    def fetch_position(symbol):
        """Fetch position for a specific symbol (or None)."""
        positions = LiveTradingComponent.fetch_positions()
        for pos in positions:
            if pos.get("symbol", "").upper() == symbol.upper():
                return pos
        return None

    @staticmethod
    def get_trade_headers():
        """Get headers for live trade execution (always sends X-Live-Trading-Key)."""
        headers = get_headers()
        live_key = os.getenv("LIVE_TRADING_KEY", "")
        if live_key:
            headers["X-Live-Trading-Key"] = live_key
        return headers

    # Safety limits for live trading (configurable via env vars)
    MAX_ORDER_SHARES = int(os.getenv("MAX_LIVE_ORDER_SHARES", "1000"))
    MAX_ORDER_VALUE = float(os.getenv("MAX_LIVE_ORDER_VALUE", "50000"))

    @staticmethod
    def execute_trade(symbol, action, shares):
        """Execute a live trade via broker API with safety guards."""
        # Frontend safety: enforce max order size
        if shares > LiveTradingComponent.MAX_ORDER_SHARES:
            return {"success": False, "error": f"Order exceeds max shares limit ({LiveTradingComponent.MAX_ORDER_SHARES})"}
        try:
            trade_data = {
                "symbol": symbol.upper(),
                "action": action,
                "shares": shares,
                "notes": "Executed from main dashboard (LIVE)",
            }
            response = requests.post(
                f"{API_BASE}/api/broker/execute",
                json=trade_data,
                headers=LiveTradingComponent.get_trade_headers(),
                timeout=API_TIMEOUT
            )
            if response.status_code != 200:
                return {"success": False, "error": f"HTTP {response.status_code}: {response.text[:200]}"}
            return response.json()
        except requests.exceptions.RequestException as e:
            return {"success": False, "error": str(e)}

    @staticmethod
    def create_panel(account, position, symbol, analysis=None):
        """Build the live trading card for the dashboard."""
        if account is None:
            return dbc.Card([
                dbc.CardHeader(html.H5("Live Trading", className="mb-0")),
                dbc.CardBody(
                    dbc.Alert(
                        "Could not connect to broker. Ensure Alpaca env vars are set and the backend is running.",
                        color="warning",
                    )
                ),
            ])

        buying_power = float(account.get("buying_power", 0))
        portfolio_value = float(account.get("portfolio_value", 0))
        cash = float(account.get("cash", 0))

        # --- Warning banner ---
        warning_banner = dbc.Alert(
            "Real money. Trades execute against your brokerage account.",
            color="danger",
            className="mb-3 fw-bold text-center",
        )

        # --- Account summary ---
        account_row = dbc.Row([
            dbc.Col([
                html.Small("Buying Power", className="text-muted d-block"),
                html.H5(f"${buying_power:,.2f}", className="text-success mb-0"),
            ], md=4, className="text-center"),
            dbc.Col([
                html.Small("Portfolio Value", className="text-muted d-block"),
                html.H5(f"${portfolio_value:,.2f}", className="text-info mb-0"),
            ], md=4, className="text-center"),
            dbc.Col([
                html.Small("Cash", className="text-muted d-block"),
                html.H5(f"${cash:,.2f}", className="text-warning mb-0"),
            ], md=4, className="text-center"),
        ], className="mb-3")

        # --- Current position in this symbol ---
        if position:
            qty = float(position.get("qty", 0))
            avg_entry = float(position.get("avg_entry_price", 0))
            market_value = float(position.get("market_value", 0))
            unrealized_pl = float(position.get("unrealized_pl", 0))
            unrealized_plpc = float(position.get("unrealized_plpc", 0)) * 100
            current_price = float(position.get("current_price", 0))

            pnl_color = "success" if unrealized_pl >= 0 else "danger"
            pnl_icon = "+" if unrealized_pl >= 0 else ""

            position_section = dbc.Row([
                dbc.Col([
                    html.Small("Shares", className="text-muted d-block"),
                    html.Span(f"{qty:g}", className="fw-bold"),
                ], width=2, className="text-center"),
                dbc.Col([
                    html.Small("Avg Cost", className="text-muted d-block"),
                    html.Span(f"${avg_entry:.2f}"),
                ], width=2, className="text-center"),
                dbc.Col([
                    html.Small("Current", className="text-muted d-block"),
                    html.Span(f"${current_price:.2f}"),
                ], width=2, className="text-center"),
                dbc.Col([
                    html.Small("Mkt Value", className="text-muted d-block"),
                    html.Span(f"${market_value:,.2f}"),
                ], width=3, className="text-center"),
                dbc.Col([
                    html.Small("Unrealized P&L", className="text-muted d-block"),
                    html.Span(
                        f"{pnl_icon}${unrealized_pl:,.2f} ({pnl_icon}{unrealized_plpc:.1f}%)",
                        className=f"text-{pnl_color} fw-bold",
                    ),
                ], width=3, className="text-center"),
            ], className="mb-3")

            position_block = html.Div([
                html.H6(f"Position in {symbol}", className="mb-2"),
                position_section,
                html.Hr(),
            ])
        else:
            position_block = html.Div([
                html.P(f"No open position in {symbol}", className="text-muted small mb-2"),
                html.Hr(),
            ])

        # --- Signal-based suggestion ---
        suggestion = ""
        if analysis:
            signal = analysis.get("overall_signal", "")
            confidence = analysis.get("overall_confidence", 0) * 100
            if signal:
                suggestion = f"Analysis signal: {signal} at {confidence:.0f}% confidence"

        suggestion_block = html.P(suggestion, className="small text-info mb-2") if suggestion else None

        # --- Confirmation checkbox ---
        confirm_check = dbc.Checkbox(
            id="live-trade-confirm-check",
            label="I confirm this is a real-money trade",
            value=False,
            className="mb-3 text-danger fw-bold",
        )

        # --- Trade controls ---
        trade_controls = dbc.Row([
            dbc.Col([
                dbc.InputGroup([
                    dbc.InputGroupText("Shares"),
                    dbc.Input(
                        id="live-trade-shares",
                        type="number",
                        value=1,
                        min=1,
                        step=1,
                    ),
                ], size="sm"),
            ], md=4),
            dbc.Col([
                dbc.Button(
                    "Buy",
                    id="live-trade-buy-btn",
                    color="success",
                    className="w-100",
                    size="sm",
                    disabled=True,
                ),
            ], md=4),
            dbc.Col([
                dbc.Button(
                    "Sell",
                    id="live-trade-sell-btn",
                    color="danger",
                    className="w-100",
                    size="sm",
                    disabled=True,
                ),
            ], md=4),
        ])

        return dbc.Card([
            dbc.CardHeader(
                html.Div([
                    html.H5("Live Trading", className="mb-0 d-inline"),
                    dbc.Badge("LIVE", color="danger", className="ms-2"),
                ], className="d-flex align-items-center")
            ),
            dbc.CardBody([
                warning_banner,
                account_row,
                html.Hr(),
                position_block,
                suggestion_block,
                confirm_check,
                trade_controls,
            ]),
        ])
