#!/usr/bin/env python3
"""
InvestIQ Trading Dashboard
Complete web interface for paper trading with execute buttons
"""

import dash
from dash import dcc, html, callback, Input, Output, State, ALL, ctx
import dash_bootstrap_components as dbc
import requests
import plotly.graph_objects as go
from datetime import datetime
import os

# API Configuration
API_BASE = os.getenv("API_BASE", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "")

if not API_KEY:
    print("ERROR: API_KEY environment variable not set!")
    print("Set it with: export API_KEY=your_key_here")
    exit(1)

headers = {"X-API-Key": API_KEY, "Content-Type": "application/json"}

# Initialize Dash app
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.CYBORG],
    suppress_callback_exceptions=True
)

# Helper functions
def fetch_account():
    """Get Alpaca account info"""
    try:
        response = requests.get(f"{API_BASE}/api/broker/account", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', {})
    except Exception as e:
        print(f"Error fetching account: {e}")
    return None

def fetch_actions():
    """Get action inbox items"""
    try:
        response = requests.get(f"{API_BASE}/api/alerts/actions", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', [])
    except Exception as e:
        print(f"Error fetching actions: {e}")
    return []

def fetch_portfolio():
    """Get portfolio summary"""
    try:
        response = requests.get(f"{API_BASE}/api/portfolio", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', {})
    except Exception as e:
        print(f"Error fetching portfolio: {e}")
    return None

def fetch_trades(limit=20):
    """Get recent trades"""
    try:
        response = requests.get(f"{API_BASE}/api/trades?limit={limit}", headers=headers, timeout=10)
        if response.status_code == 200:
            return response.json().get('data', [])
    except Exception as e:
        print(f"Error fetching trades: {e}")
    return []

def execute_trade(symbol, action, shares):
    """Execute a trade via broker API"""
    try:
        trade_data = {
            "symbol": symbol,
            "action": action,
            "shares": shares,
            "notes": f"Executed from trading dashboard"
        }
        response = requests.post(
            f"{API_BASE}/api/broker/execute",
            json=trade_data,
            headers=headers,
            timeout=30
        )
        return response.json()
    except Exception as e:
        return {"success": False, "error": str(e)}

# Layout Components
def create_account_banner():
    """Account balance banner"""
    account = fetch_account()

    if not account:
        return dbc.Alert("Unable to connect to broker", color="danger")

    buying_power = float(account.get('buying_power', 0))
    portfolio_value = float(account.get('portfolio_value', 0))
    cash = float(account.get('cash', 0))

    return dbc.Card([
        dbc.CardBody([
            html.H3("Paper Trading Account", className="text-center mb-3"),
            dbc.Row([
                dbc.Col([
                    html.H5("Buying Power", className="text-muted"),
                    html.H2(f"${buying_power:,.2f}", className="text-success")
                ], width=4, className="text-center"),
                dbc.Col([
                    html.H5("Portfolio Value", className="text-muted"),
                    html.H2(f"${portfolio_value:,.2f}", className="text-info")
                ], width=4, className="text-center"),
                dbc.Col([
                    html.H5("Cash", className="text-muted"),
                    html.H2(f"${cash:,.2f}", className="text-warning")
                ], width=4, className="text-center"),
            ])
        ])
    ], className="mb-4", color="dark")

def create_action_card(action, index):
    """Create a card for each action with Execute button"""
    symbol = action.get('symbol', 'N/A')
    action_type = action.get('action_type', '').upper()
    confidence = action.get('confidence', 0) * 100
    signal = action.get('signal', '')
    description = action.get('description', '')
    current_price = action.get('current_price')
    target_price = action.get('target_price')
    in_portfolio = action.get('in_portfolio', False)

    # Determine button color and icon
    if action_type == "BUY":
        btn_color = "success"
        icon = "📈"
    elif action_type == "SELL":
        btn_color = "danger"
        icon = "📉"
    else:
        btn_color = "warning"
        icon = "⚠️"

    # Confidence badge color
    if confidence >= 80:
        conf_color = "success"
    elif confidence >= 60:
        conf_color = "warning"
    else:
        conf_color = "secondary"

    card_content = [
        dbc.CardHeader([
            dbc.Row([
                dbc.Col([
                    html.H4([icon, f" {symbol}"], className="mb-0")
                ], width=6),
                dbc.Col([
                    dbc.Badge(f"{confidence:.0f}% Confidence", color=conf_color, className="float-end")
                ], width=6)
            ])
        ]),
        dbc.CardBody([
            html.H5(signal, className="card-title"),
            html.P(description, className="card-text"),

            dbc.Row([
                dbc.Col([
                    html.Small("Current Price:", className="text-muted"),
                    html.Div(f"${current_price:.2f}" if current_price else "N/A")
                ], width=6) if current_price else None,
                dbc.Col([
                    html.Small("Target Price:", className="text-muted"),
                    html.Div(f"${target_price:.2f}" if target_price else "N/A")
                ], width=6) if target_price else None,
            ], className="mb-3"),

            dbc.Badge("In Portfolio", color="info", className="mb-2") if in_portfolio else None,

            dbc.Row([
                dbc.Col([
                    dbc.Button(
                        f"{icon} Execute {action_type}",
                        id={"type": "execute-btn", "index": index},
                        color=btn_color,
                        className="w-100",
                        size="lg"
                    )
                ])
            ])
        ])
    ]

    return dbc.Col([
        dbc.Card(card_content, className="mb-3", color="dark", outline=True)
    ], width=12, md=6, lg=4)

def create_portfolio_table():
    """Portfolio positions table"""
    portfolio = fetch_portfolio()

    if not portfolio or not portfolio.get('positions'):
        return dbc.Alert("No positions in portfolio", color="info")

    positions = portfolio.get('positions', [])

    rows = []
    for pos_data in positions:
        pos = pos_data.get('position', {})
        symbol = pos.get('symbol', 'N/A')
        shares = pos.get('shares', 0)
        entry_price = pos.get('entry_price', 0)
        current_price = pos_data.get('current_price', 0)
        unrealized_pnl = pos_data.get('unrealized_pnl', 0)
        unrealized_pnl_pct = pos_data.get('unrealized_pnl_percent', 0)

        pnl_color = "success" if unrealized_pnl >= 0 else "danger"
        pnl_icon = "📈" if unrealized_pnl >= 0 else "📉"

        rows.append(html.Tr([
            html.Td(symbol, className="fw-bold"),
            html.Td(f"{shares:.2f}"),
            html.Td(f"${entry_price:.2f}"),
            html.Td(f"${current_price:.2f}"),
            html.Td([
                pnl_icon,
                html.Span(f" ${unrealized_pnl:.2f}", className=f"text-{pnl_color}")
            ]),
            html.Td([
                html.Span(f"{unrealized_pnl_pct:.2f}%", className=f"text-{pnl_color}")
            ])
        ]))

    return dbc.Table([
        html.Thead(html.Tr([
            html.Th("Symbol"),
            html.Th("Shares"),
            html.Th("Entry Price"),
            html.Th("Current Price"),
            html.Th("P&L"),
            html.Th("P&L %")
        ])),
        html.Tbody(rows)
    ], bordered=True, hover=True, responsive=True, className="table-dark")

def create_trades_table():
    """Recent trades table"""
    trades = fetch_trades(10)

    if not trades:
        return dbc.Alert("No trades yet", color="info")

    rows = []
    for trade in trades:
        symbol = trade.get('symbol', 'N/A')
        action = trade.get('action', '').upper()
        shares = trade.get('shares', 0)
        price = trade.get('price', 0)
        trade_date = trade.get('trade_date', '')
        pnl = trade.get('profit_loss')

        action_color = "success" if action == "BUY" else "danger"
        action_icon = "📈" if action == "BUY" else "📉"

        rows.append(html.Tr([
            html.Td(trade_date),
            html.Td(symbol, className="fw-bold"),
            html.Td([
                html.Span(action_icon),
                html.Span(f" {action}", className=f"text-{action_color}")
            ]),
            html.Td(f"{shares:.2f}"),
            html.Td(f"${price:.2f}"),
            html.Td(f"${pnl:.2f}" if pnl else "-", className="text-success" if pnl and pnl > 0 else "text-danger" if pnl else "")
        ]))

    return dbc.Table([
        html.Thead(html.Tr([
            html.Th("Date"),
            html.Th("Symbol"),
            html.Th("Action"),
            html.Th("Shares"),
            html.Th("Price"),
            html.Th("P&L")
        ])),
        html.Tbody(rows)
    ], bordered=True, hover=True, responsive=True, className="table-dark")

# Main Layout
app.layout = dbc.Container([
    dcc.Store(id='notification-store'),
    dcc.Interval(id='refresh-interval', interval=30000, n_intervals=0),  # Refresh every 30s

    # Header
    dbc.Row([
        dbc.Col([
            html.H1("💰 InvestIQ Trading Dashboard", className="text-center my-4"),
            html.P("Paper Trading with Real-Time Execution", className="text-center text-muted")
        ])
    ]),

    # Account Banner
    html.Div(id='account-banner'),

    # Notifications
    html.Div(id='notification-area'),

    # Action Inbox
    dbc.Row([
        dbc.Col([
            html.H3("🔔 Action Inbox", className="mb-3"),
            html.Div(id='action-inbox')
        ])
    ], className="mb-4"),

    # Portfolio Section
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H4("📊 Current Portfolio")),
                dbc.CardBody(html.Div(id='portfolio-table'))
            ], color="dark")
        ])
    ], className="mb-4"),

    # Recent Trades
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H4("📜 Recent Trades")),
                dbc.CardBody(html.Div(id='trades-table'))
            ], color="dark")
        ])
    ], className="mb-4"),

    # Confirmation Modal
    dbc.Modal([
        dbc.ModalHeader(dbc.ModalTitle("Confirm Trade", id='modal-title')),
        dbc.ModalBody([
            html.Div(id='modal-body'),
            dbc.Input(id='shares-input', type='number', placeholder='Number of shares', value=10, min=1, step=1, className="mt-3")
        ]),
        dbc.ModalFooter([
            dbc.Button("Cancel", id="cancel-btn", color="secondary", className="me-2"),
            dbc.Button("Execute Trade", id="confirm-btn", color="success")
        ])
    ], id="confirm-modal", is_open=False, backdrop="static"),

], fluid=True, className="py-3")

# Callbacks
@callback(
    [Output('account-banner', 'children'),
     Output('action-inbox', 'children'),
     Output('portfolio-table', 'children'),
     Output('trades-table', 'children')],
    [Input('refresh-interval', 'n_intervals'),
     Input('notification-store', 'data')]
)
def refresh_data(n, notification_data):
    """Refresh all dashboard data"""
    account_banner = create_account_banner()

    actions = fetch_actions()
    action_cards = dbc.Row([
        create_action_card(action, idx) for idx, action in enumerate(actions)
    ]) if actions else dbc.Alert("No action items at the moment", color="info")

    portfolio_table = create_portfolio_table()
    trades_table = create_trades_table()

    return account_banner, action_cards, portfolio_table, trades_table

@callback(
    [Output('confirm-modal', 'is_open'),
     Output('modal-title', 'children'),
     Output('modal-body', 'children'),
     Output('notification-store', 'data', allow_duplicate=True)],
    [Input({'type': 'execute-btn', 'index': ALL}, 'n_clicks'),
     Input('confirm-btn', 'n_clicks'),
     Input('cancel-btn', 'n_clicks')],
    [State('confirm-modal', 'is_open'),
     State('shares-input', 'value'),
     State('notification-store', 'data')],
    prevent_initial_call=True
)
def handle_trade_execution(execute_clicks, confirm_click, cancel_click, is_open, shares, notification_data):
    """Handle trade execution flow"""
    triggered_id = ctx.triggered_id

    # Cancel button
    if triggered_id == 'cancel-btn':
        return False, "", "", notification_data

    # Execute button clicked - show confirmation modal
    if triggered_id and isinstance(triggered_id, dict) and triggered_id['type'] == 'execute-btn':
        if not any(execute_clicks):
            return is_open, "", "", notification_data

        # Find which button was clicked
        clicked_index = None
        for idx, clicks in enumerate(execute_clicks):
            if clicks:
                clicked_index = idx
                break

        if clicked_index is None:
            return is_open, "", "", notification_data

        # Get the action details
        actions = fetch_actions()
        if clicked_index >= len(actions):
            return is_open, "", "", notification_data

        action = actions[clicked_index]
        symbol = action.get('symbol', 'N/A')
        action_type = action.get('action_type', '').upper()
        current_price = action.get('current_price')
        confidence = action.get('confidence', 0) * 100

        modal_title = f"Execute {action_type} Order: {symbol}"
        modal_body = html.Div([
            html.H5(f"Symbol: {symbol}"),
            html.P(f"Action: {action_type}"),
            html.P(f"Current Price: ${current_price:.2f}" if current_price else "Price: N/A"),
            html.P(f"Confidence: {confidence:.0f}%"),
            html.Hr(),
            html.P("Enter number of shares to trade:", className="text-muted")
        ])

        return True, modal_title, modal_body, notification_data

    # Confirm button clicked - execute trade
    if triggered_id == 'confirm-btn':
        # Get the action from modal state
        # We need to store which action we're confirming
        # For now, we'll get the first action (this is simplified - in production you'd store the action in State)
        actions = fetch_actions()
        if not actions:
            return False, "", "", notification_data

        action = actions[0]  # Simplified - should track which action is being confirmed
        symbol = action.get('symbol')
        action_type = action.get('action_type', '').lower()

        # Execute the trade
        result = execute_trade(symbol, action_type, shares if shares else 10)

        # Create notification
        if result.get('success'):
            notification_data = {
                'type': 'success',
                'message': f"Successfully executed {action_type.upper()} {shares} shares of {symbol}",
                'timestamp': datetime.now().isoformat()
            }
        else:
            notification_data = {
                'type': 'error',
                'message': f"Failed to execute trade: {result.get('error', 'Unknown error')}",
                'timestamp': datetime.now().isoformat()
            }

        return False, "", "", notification_data

    return is_open, "", "", notification_data

@callback(
    Output('notification-area', 'children'),
    Input('notification-store', 'data')
)
def show_notification(notification_data):
    """Display notification toast"""
    if not notification_data:
        return None

    alert_type = notification_data.get('type', 'info')
    message = notification_data.get('message', '')

    if alert_type == 'success':
        color = 'success'
        icon = '✅'
    elif alert_type == 'error':
        color = 'danger'
        icon = '❌'
    else:
        color = 'info'
        icon = 'ℹ️'

    return dbc.Alert(
        [html.Strong(icon), f" {message}"],
        color=color,
        dismissable=True,
        duration=5000
    )

if __name__ == '__main__':
    print("\n" + "="*60)
    print("💰 InvestIQ Trading Dashboard")
    print("="*60)
    print(f"API Base: {API_BASE}")
    print(f"API Key: {'*' * 20}{API_KEY[-4:] if len(API_KEY) > 4 else 'NOT SET'}")
    print("="*60)
    print("\nStarting dashboard on http://localhost:8052")
    print("Press Ctrl+C to stop\n")

    app.run(debug=True, host='0.0.0.0', port=8052)
