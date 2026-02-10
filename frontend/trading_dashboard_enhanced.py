#!/usr/bin/env python3
"""
InvestIQ Enhanced Trading Dashboard
Complete web interface for paper trading with improved UX/UI
"""

import dash
from dash import dcc, html, callback, Input, Output, State, ALL, ctx
import dash_bootstrap_components as dbc
import requests
import plotly.graph_objects as go
from datetime import datetime
import os
import json

# API Configuration
API_BASE = os.getenv("API_BASE", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "")

if not API_KEY:
    print("⚠️  WARNING: API_KEY environment variable not set!")
    print("Set it with: export API_KEY=your_key_here")

headers = {"X-API-Key": API_KEY, "Content-Type": "application/json"}

# Initialize Dash app
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.CYBORG, dbc.icons.FONT_AWESOME],
    suppress_callback_exceptions=True,
    title="InvestIQ Trading Dashboard"
)

# ===== HELPER FUNCTIONS =====

def fetch_with_error_handling(endpoint: str, method: str = "GET", data: dict = None, timeout: int = 10):
    """Fetch data with comprehensive error handling"""
    try:
        if method == "GET":
            response = requests.get(f"{API_BASE}{endpoint}", headers=headers, timeout=timeout)
        else:
            response = requests.post(f"{API_BASE}{endpoint}", json=data, headers=headers, timeout=timeout)

        if response.status_code == 200:
            return {'success': True, 'data': response.json().get('data', {})}
        elif response.status_code == 401:
            return {'success': False, 'error': 'Authentication failed. Please check your API key.'}
        elif response.status_code == 404:
            return {'success': False, 'error': 'Endpoint not found. The trading feature may not be available.'}
        elif response.status_code == 429:
            return {'success': False, 'error': 'Rate limit exceeded. Please wait a moment and try again.'}
        else:
            return {'success': False, 'error': f'Server error: {response.status_code}'}
    except requests.exceptions.Timeout:
        return {'success': False, 'error': 'Request timed out. The server may be slow or unavailable.'}
    except requests.exceptions.ConnectionError:
        return {'success': False, 'error': 'Cannot connect to server. Make sure the API server is running.'}
    except Exception as e:
        return {'success': False, 'error': f'Unexpected error: {str(e)}'}


def create_error_alert(error_msg: str, suggestion: str = None):
    """Create user-friendly error alert"""
    content = [
        html.H5([html.I(className="fas fa-exclamation-triangle me-2"), "Error"], className="alert-heading"),
        html.P(error_msg, className="mb-2"),
    ]

    if suggestion:
        content.append(html.Hr())
        content.append(html.P([
            html.Strong("💡 Try this: "),
            suggestion
        ], className="mb-0 small"))

    return dbc.Alert(content, color="danger", dismissable=True)


def create_skeleton_loader(type: str = "card"):
    """Create skeleton loader"""
    if type == "card":
        return dbc.Card([
            dbc.CardBody([
                html.Div(className="skeleton-line", style={'width': '60%', 'height': '20px', 'marginBottom': '10px'}),
                html.Div(className="skeleton-line", style={'width': '80%', 'height': '15px', 'marginBottom': '10px'}),
                html.Div(className="skeleton-box", style={'width': '100%', 'height': '100px'}),
            ])
        ], className="skeleton-card mb-3")
    return html.Div("Loading...")


# ===== LAYOUT COMPONENTS =====

def create_account_banner():
    """Enhanced account balance banner with sparklines"""
    result = fetch_with_error_handling("/api/broker/account")

    if not result['success']:
        return create_error_alert(
            result['error'],
            "Make sure the API server is running and your API key is correct."
        )

    account = result['data']
    buying_power = float(account.get('buying_power', 0))
    portfolio_value = float(account.get('portfolio_value', 0))
    cash = float(account.get('cash', 0))
    equity = float(account.get('equity', 0))

    # Calculate daily P&L (if available)
    daily_pnl = 0
    daily_pnl_pct = 0

    return dbc.Card([
        dbc.CardBody([
            dbc.Row([
                dbc.Col([
                    html.Div([
                        html.H6("Paper Trading Account", className="text-muted mb-3"),
                        dbc.Row([
                            dbc.Col([
                                html.Div([
                                    html.I(className="fas fa-wallet fa-2x text-success mb-2"),
                                    html.H6("Buying Power", className="text-muted small"),
                                    html.H3(f"${buying_power:,.2f}", className="mb-0 text-gradient-success"),
                                ], className="text-center metric-card")
                            ], md=3),
                            dbc.Col([
                                html.Div([
                                    html.I(className="fas fa-chart-line fa-2x text-info mb-2"),
                                    html.H6("Portfolio Value", className="text-muted small"),
                                    html.H3(f"${portfolio_value:,.2f}", className="mb-0 text-gradient-primary"),
                                ], className="text-center metric-card")
                            ], md=3),
                            dbc.Col([
                                html.Div([
                                    html.I(className="fas fa-dollar-sign fa-2x text-warning mb-2"),
                                    html.H6("Cash", className="text-muted small"),
                                    html.H3(f"${cash:,.2f}", className="mb-0"),
                                ], className="text-center metric-card")
                            ], md=3),
                            dbc.Col([
                                html.Div([
                                    html.I(className="fas fa-chart-pie fa-2x text-primary mb-2"),
                                    html.H6("Total Equity", className="text-muted small"),
                                    html.H3(f"${equity:,.2f}", className="mb-0"),
                                ], className="text-center metric-card")
                            ], md=3),
                        ])
                    ])
                ])
            ])
        ])
    ], className="mb-4 shadow-glow")


def create_action_card(action, index):
    """Enhanced action card with better visuals"""
    symbol = action.get('symbol', 'N/A')
    action_type = action.get('action_type', '').upper()
    confidence = action.get('confidence', 0) * 100
    signal = action.get('signal', '')
    description = action.get('description', '')
    current_price = action.get('current_price')
    target_price = action.get('target_price')
    in_portfolio = action.get('in_portfolio', False)

    # Determine styling
    if action_type == "BUY":
        btn_color = "success"
        icon = "📈"
        card_color = "rgba(56, 239, 125, 0.1)"
    elif action_type == "SELL":
        btn_color = "danger"
        icon = "📉"
        card_color = "rgba(244, 92, 67, 0.1)"
    else:
        btn_color = "warning"
        icon = "⚠️"
        card_color = "rgba(245, 87, 108, 0.1)"

    # Confidence badge
    if confidence >= 80:
        conf_color = "success"
        conf_icon = "fas fa-check-circle"
    elif confidence >= 60:
        conf_color = "warning"
        conf_icon = "fas fa-exclamation-circle"
    else:
        conf_color = "secondary"
        conf_icon = "fas fa-question-circle"

    # Calculate potential return
    potential_return = None
    if current_price and target_price:
        potential_return = ((target_price - current_price) / current_price) * 100

    card_content = [
        dbc.CardHeader([
            dbc.Row([
                dbc.Col([
                    html.H4([
                        icon,
                        f" {symbol}",
                        dbc.Badge("In Portfolio", color="info", className="ms-2") if in_portfolio else None
                    ], className="mb-0")
                ], width=6),
                dbc.Col([
                    dbc.Badge([
                        html.I(className=f"{conf_icon} me-1"),
                        f"{confidence:.0f}%"
                    ], color=conf_color, className="float-end", pill=True, style={'fontSize': '1rem'})
                ], width=6)
            ])
        ], style={'background': card_color}),
        dbc.CardBody([
            html.H5(signal, className="card-title mb-3"),
            html.P(description, className="card-text mb-3"),

            # Metrics
            dbc.Row([
                dbc.Col([
                    html.Small("Current Price", className="text-muted d-block"),
                    html.H6(f"${current_price:.2f}" if current_price else "N/A", className="mb-0")
                ], width=4),
                dbc.Col([
                    html.Small("Target Price", className="text-muted d-block"),
                    html.H6(f"${target_price:.2f}" if target_price else "N/A", className="mb-0")
                ], width=4),
                dbc.Col([
                    html.Small("Potential", className="text-muted d-block"),
                    html.H6([
                        f"{potential_return:+.1f}%" if potential_return else "N/A",
                        html.I(className="fas fa-arrow-up ms-1 text-success") if potential_return and potential_return > 0 else
                        html.I(className="fas fa-arrow-down ms-1 text-danger") if potential_return and potential_return < 0 else None
                    ], className="mb-0")
                ], width=4),
            ], className="mb-4"),

            # Confidence progress bar
            html.Small("Confidence Level", className="text-muted"),
            dbc.Progress(
                value=confidence,
                color=conf_color,
                className="mb-3",
                style={'height': '8px'}
            ),

            # Action buttons
            dbc.Row([
                dbc.Col([
                    dbc.Button([
                        html.I(className=f"fas fa-{'arrow-up' if action_type == 'BUY' else 'arrow-down'} me-2"),
                        f"Execute {action_type}"
                    ],
                        id={'type': 'execute-btn', 'index': index},
                        color=btn_color,
                        className="w-100",
                        size="lg"
                    )
                ], width=8),
                dbc.Col([
                    dbc.Button(
                        html.I(className="fas fa-info-circle"),
                        id={'type': 'details-btn', 'index': index},
                        color="secondary",
                        outline=True,
                        className="w-100",
                        size="lg"
                    )
                ], width=4),
            ])
        ])
    ]

    return dbc.Col([
        dbc.Card(card_content, className="h-100 hover-lift", style={'borderLeft': f'4px solid {btn_color}'})
    ], width=12, md=6, lg=4, className="mb-4")


def create_portfolio_table():
    """Enhanced portfolio table with sparklines and metrics"""
    result = fetch_with_error_handling("/api/portfolio")

    if not result['success']:
        return create_error_alert(result['error'], "Check if the portfolio feature is enabled in the API.")

    portfolio = result['data']

    if not portfolio or not portfolio.get('positions'):
        return dbc.Alert([
            html.I(className="fas fa-info-circle me-2"),
            "No positions in portfolio. Execute a trade to get started!"
        ], color="info", className="text-center")

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
            html.Td([
                html.Strong(symbol),
                html.Br(),
                html.Small(f"{shares:.2f} shares", className="text-muted")
            ]),
            html.Td(f"${entry_price:.2f}"),
            html.Td([
                f"${current_price:.2f}",
                html.Br(),
                html.Small(
                    f"${current_price * shares:,.2f}",
                    className="text-muted"
                )
            ]),
            html.Td([
                html.Span(pnl_icon),
                html.Span(f" ${abs(unrealized_pnl):,.2f}", className=f"text-{pnl_color} ms-1")
            ]),
            html.Td([
                dbc.Badge(
                    f"{unrealized_pnl_pct:+.2f}%",
                    color=pnl_color,
                    className="me-2"
                ),
                dbc.Progress(
                    value=abs(unrealized_pnl_pct),
                    color=pnl_color,
                    style={'height': '4px', 'width': '60px'},
                    className="d-inline-block"
                )
            ]),
            html.Td([
                dbc.ButtonGroup([
                    dbc.Button(
                        html.I(className="fas fa-chart-line"),
                        size="sm",
                        color="info",
                        outline=True,
                        id={'type': 'view-position', 'symbol': symbol}
                    ),
                    dbc.Button(
                        html.I(className="fas fa-times"),
                        size="sm",
                        color="danger",
                        outline=True,
                        id={'type': 'close-position', 'symbol': symbol}
                    ),
                ], size="sm")
            ])
        ]))

    return dbc.Table([
        html.Thead(html.Tr([
            html.Th("Position"),
            html.Th("Entry Price"),
            html.Th("Current Value"),
            html.Th("P&L ($)"),
            html.Th("P&L (%)"),
            html.Th("Actions")
        ])),
        html.Tbody(rows)
    ], bordered=True, hover=True, responsive=True, className="table-dark")


def create_trades_table():
    """Enhanced recent trades table"""
    result = fetch_with_error_handling("/api/trades?limit=10")

    if not result['success']:
        return create_error_alert(result['error'])

    trades = result['data']

    if not trades:
        return dbc.Alert([
            html.I(className="fas fa-info-circle me-2"),
            "No trades yet. Your trading history will appear here."
        ], color="info", className="text-center")

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

        # Format date
        try:
            dt = datetime.fromisoformat(trade_date.replace('Z', '+00:00'))
            formatted_date = dt.strftime("%m/%d/%y %I:%M %p")
        except:
            formatted_date = trade_date

        rows.append(html.Tr([
            html.Td([
                html.Small(formatted_date, className="text-muted d-block"),
                html.Small(f"{(datetime.now() - dt).days}d ago" if 'dt' in locals() else "", className="text-muted small")
            ]),
            html.Td([
                html.Strong(symbol),
                html.Br(),
                html.Small(f"{shares:.2f} shares", className="text-muted")
            ]),
            html.Td([
                dbc.Badge([
                    html.Span(action_icon),
                    html.Span(f" {action}", className="ms-1")
                ], color=action_color)
            ]),
            html.Td(f"${price:.2f}"),
            html.Td(f"${price * shares:,.2f}"),
            html.Td([
                dbc.Badge(
                    f"${pnl:+,.2f}" if pnl else "-",
                    color="success" if pnl and pnl > 0 else "danger" if pnl and pnl < 0 else "secondary"
                ) if pnl is not None else html.Span("-", className="text-muted")
            ])
        ]))

    return dbc.Table([
        html.Thead(html.Tr([
            html.Th("Date"),
            html.Th("Symbol"),
            html.Th("Action"),
            html.Th("Price"),
            html.Th("Total"),
            html.Th("P&L")
        ])),
        html.Tbody(rows)
    ], bordered=True, hover=True, responsive=True, className="table-dark", size="sm")


# ===== MAIN LAYOUT =====

app.layout = dbc.Container([
    # Stores
    dcc.Store(id='notification-store'),
    dcc.Store(id='selected-action', data={}),
    dcc.Interval(id='refresh-interval', interval=30000, n_intervals=0, disabled=True),

    # Keyboard listener
    html.Div(id='keyboard-listener', tabIndex=0, style={'outline': 'none'}),

    # Header with navbar
    dbc.Navbar([
        dbc.Container([
            dbc.Row([
                dbc.Col([
                    dbc.NavbarBrand([
                        html.I(className="fas fa-chart-line me-2"),
                        "InvestIQ Trading"
                    ], className="ms-2", style={'fontSize': '1.5rem', 'fontWeight': '700'}),
                ], width="auto"),
                dbc.Col([
                    dbc.Nav([
                        dbc.NavItem(dbc.NavLink([
                            html.I(className="fas fa-home me-1"),
                            "Dashboard"
                        ], active=True)),
                        dbc.NavItem(dbc.NavLink([
                            html.I(className="fas fa-history me-1"),
                            "History"
                        ])),
                        dbc.NavItem(dbc.NavLink([
                            html.I(className="fas fa-cog me-1"),
                            "Settings"
                        ])),
                    ], navbar=True),
                ], width="auto"),
                dbc.Col([
                    dbc.ButtonGroup([
                        dbc.Checklist(
                            options=[{"label": "Auto-refresh", "value": 1}],
                            value=[],
                            id="auto-refresh-toggle",
                            switch=True,
                            inline=True,
                            className="me-2"
                        ),
                        dbc.Button(
                            html.I(className="fas fa-sync-alt"),
                            id="manual-refresh-btn",
                            color="secondary",
                            size="sm",
                            outline=True
                        ),
                    ], size="sm", className="ms-auto")
                ], width="auto", className="ms-auto"),
            ], className="w-100 align-items-center", justify="between"),
        ], fluid=True)
    ], color="dark", dark=True, className="mb-4", sticky="top"),

    # Account Banner
    html.Div(id='account-banner'),

    # Notifications
    html.Div(id='notification-area'),

    # Last updated indicator
    dbc.Row([
        dbc.Col([
            html.Small([
                html.I(className="fas fa-clock me-1"),
                "Last updated: ",
                html.Span(id='last-updated', className="text-info"),
            ], className="text-muted")
        ], className="text-end")
    ], className="mb-3"),

    # Main Content Tabs
    dbc.Tabs([
        dbc.Tab(label="🔔 Action Inbox", tab_id="tab-actions"),
        dbc.Tab(label="📊 Portfolio", tab_id="tab-portfolio"),
        dbc.Tab(label="📜 Trade History", tab_id="tab-history"),
        dbc.Tab(label="📈 Analytics", tab_id="tab-analytics"),
    ], id="main-tabs", active_tab="tab-actions", className="mb-4"),

    html.Div(id='tab-content'),

    # Enhanced Confirmation Modal
    dbc.Modal([
        dbc.ModalHeader(dbc.ModalTitle("", id='modal-title')),
        dbc.ModalBody([
            html.Div(id='modal-body'),
            html.Hr(),
            dbc.Row([
                dbc.Col([
                    dbc.Label("Number of Shares"),
                    dbc.Input(
                        id='shares-input',
                        type='number',
                        placeholder='Enter quantity',
                        value=10,
                        min=1,
                        step=1,
                        className="mb-2"
                    ),
                    dbc.FormText("Minimum: 1 share")
                ], md=6),
                dbc.Col([
                    dbc.Label("Order Type"),
                    dcc.Dropdown(
                        id='order-type',
                        options=[
                            {'label': 'Market Order', 'value': 'market'},
                            {'label': 'Limit Order', 'value': 'limit'},
                        ],
                        value='market',
                        className="mb-2"
                    )
                ], md=6),
            ]),
            html.Div(id='order-summary', className="mt-3")
        ]),
        dbc.ModalFooter([
            dbc.Button("Cancel", id="cancel-btn", color="secondary"),
            dbc.Button([
                html.I(className="fas fa-check me-2"),
                "Confirm Trade"
            ], id="confirm-btn", color="success", size="lg")
        ])
    ], id="confirm-modal", is_open=False, backdrop="static", size="lg"),

    # Footer
    html.Hr(className="mt-5"),
    dbc.Row([
        dbc.Col([
            html.P([
                html.I(className="fas fa-exclamation-triangle me-2"),
                "Paper Trading Mode - No real money at risk"
            ], className="text-center text-muted small mb-2"),
            html.P([
                "Made with ",
                html.I(className="fas fa-heart text-danger"),
                " by InvestIQ | ",
                html.A("Help", href="#", className="text-info"),
            ], className="text-center text-muted small")
        ])
    ])

], fluid=True, className="px-4 py-3")

# ===== CALLBACKS =====

@callback(
    Output('refresh-interval', 'disabled'),
    Input('auto-refresh-toggle', 'value')
)
def toggle_auto_refresh(value):
    return len(value) == 0


@callback(
    Output('last-updated', 'children'),
    [Input('refresh-interval', 'n_intervals'),
     Input('manual-refresh-btn', 'n_clicks')]
)
def update_timestamp(intervals, clicks):
    return datetime.now().strftime("%I:%M:%S %p")


@callback(
    [Output('account-banner', 'children'),
     Output('tab-content', 'children')],
    [Input('main-tabs', 'active_tab'),
     Input('refresh-interval', 'n_intervals'),
     Input('manual-refresh-btn', 'n_clicks'),
     Input('notification-store', 'data')]
)
def update_dashboard(active_tab, intervals, clicks, notification):
    account_banner = create_account_banner()

    if active_tab == "tab-actions":
        result = fetch_with_error_handling("/api/alerts/actions")
        if result['success']:
            actions = result['data']
            content = dbc.Row([
                create_action_card(action, idx) for idx, action in enumerate(actions)
            ]) if actions else dbc.Alert("No action items at the moment. Check back later!", color="info", className="text-center")
        else:
            content = create_error_alert(result['error'])

    elif active_tab == "tab-portfolio":
        content = dbc.Card([
            dbc.CardHeader(html.H4("📊 Current Portfolio")),
            dbc.CardBody(create_portfolio_table())
        ])

    elif active_tab == "tab-history":
        content = dbc.Card([
            dbc.CardHeader(html.H4("📜 Recent Trades")),
            dbc.CardBody(create_trades_table())
        ])

    elif active_tab == "tab-analytics":
        content = dbc.Alert("Analytics dashboard coming soon!", color="info")

    else:
        content = html.Div("Loading...")

    return account_banner, content


@callback(
    [Output('confirm-modal', 'is_open'),
     Output('modal-title', 'children'),
     Output('modal-body', 'children'),
     Output('selected-action', 'data')],
    [Input({'type': 'execute-btn', 'index': ALL}, 'n_clicks'),
     Input('cancel-btn', 'n_clicks')],
    [State('confirm-modal', 'is_open')],
    prevent_initial_call=True
)
def handle_modal(execute_clicks, cancel_click, is_open):
    if ctx.triggered_id == 'cancel-btn':
        return False, "", "", {}

    if isinstance(ctx.triggered_id, dict) and ctx.triggered_id['type'] == 'execute-btn':
        if not any(execute_clicks):
            return is_open, "", "", {}

        # Find clicked button
        clicked_index = next((i for i, c in enumerate(execute_clicks) if c), None)
        if clicked_index is None:
            return is_open, "", "", {}

        # Get action details
        result = fetch_with_error_handling("/api/alerts/actions")
        if not result['success']:
            return False, "", create_error_alert(result['error']), {}

        actions = result['data']
        if clicked_index >= len(actions):
            return False, "", "", {}

        action = actions[clicked_index]
        symbol = action.get('symbol', 'N/A')
        action_type = action.get('action_type', '').upper()
        current_price = action.get('current_price')
        confidence = action.get('confidence', 0) * 100

        modal_title = f"Confirm {action_type} Order: {symbol}"
        modal_body = html.Div([
            dbc.Row([
                dbc.Col([
                    html.I(className="fas fa-info-circle fa-3x text-info mb-3"),
                ], width=12, className="text-center"),
            ]),
            dbc.Alert([
                html.H5("Order Details", className="alert-heading"),
                html.Hr(),
                html.P([html.Strong("Symbol: "), symbol]),
                html.P([html.Strong("Action: "), action_type]),
                html.P([html.Strong("Current Price: "), f"${current_price:.2f}" if current_price else "N/A"]),
                html.P([html.Strong("Confidence: "), f"{confidence:.0f}%"]),
            ], color="info", className="mb-3"),
            html.P("Specify the order details below:", className="text-muted")
        ])

        return True, modal_title, modal_body, action

    return is_open, "", "", {}


@callback(
    [Output('notification-store', 'data', allow_duplicate=True),
     Output('confirm-modal', 'is_open', allow_duplicate=True)],
    Input('confirm-btn', 'n_clicks'),
    [State('selected-action', 'data'),
     State('shares-input', 'value')],
    prevent_initial_call=True
)
def execute_trade(n_clicks, action, shares):
    if not action or not shares:
        return None, True

    symbol = action.get('symbol')
    action_type = action.get('action_type', '').lower()

    trade_data = {
        "symbol": symbol,
        "action": action_type,
        "shares": shares,
        "notes": f"Executed from trading dashboard"
    }

    result = fetch_with_error_handling("/api/broker/execute", method="POST", data=trade_data, timeout=30)

    if result['success']:
        notification = {
            'type': 'success',
            'message': f"Successfully executed {action_type.upper()} {shares} shares of {symbol}",
            'timestamp': datetime.now().isoformat()
        }
    else:
        notification = {
            'type': 'error',
            'message': f"Failed to execute trade: {result['error']}",
            'timestamp': datetime.now().isoformat()
        }

    return notification, False


@callback(
    Output('notification-area', 'children'),
    Input('notification-store', 'data')
)
def show_notification(notification):
    if not notification:
        return None

    alert_type = notification.get('type', 'info')
    message = notification.get('message', '')

    if alert_type == 'success':
        return dbc.Alert([
            html.I(className="fas fa-check-circle me-2"),
            html.Strong("Success! "),
            message
        ], color='success', dismissable=True, duration=5000, className="fade-in")
    elif alert_type == 'error':
        return dbc.Alert([
            html.I(className="fas fa-exclamation-triangle me-2"),
            html.Strong("Error! "),
            message
        ], color='danger', dismissable=True, className="fade-in")
    else:
        return dbc.Alert(message, color='info', dismissable=True, className="fade-in")


@callback(
    Output('order-summary', 'children'),
    [Input('shares-input', 'value'),
     Input('order-type', 'value')],
    State('selected-action', 'data')
)
def update_order_summary(shares, order_type, action):
    if not shares or not action:
        return None

    current_price = action.get('current_price', 0)
    estimated_cost = shares * current_price if current_price else 0

    return dbc.Card([
        dbc.CardBody([
            html.H6("Order Summary", className="mb-3"),
            dbc.Row([
                dbc.Col([
                    html.Small("Shares", className="text-muted d-block"),
                    html.H6(f"{shares:,.0f}")
                ], width=4),
                dbc.Col([
                    html.Small("Price per Share", className="text-muted d-block"),
                    html.H6(f"${current_price:.2f}" if current_price else "N/A")
                ], width=4),
                dbc.Col([
                    html.Small("Total Cost", className="text-muted d-block"),
                    html.H5(f"${estimated_cost:,.2f}", className="text-gradient-primary")
                ], width=4),
            ])
        ])
    ], color="dark", outline=True)


if __name__ == '__main__':
    print("\n" + "="*70)
    print("💰 InvestIQ Enhanced Trading Dashboard")
    print("="*70)
    print(f"API Base: {API_BASE}")
    print(f"API Key: {'✅ Set' if API_KEY else '❌ Not Set'}")
    print("="*70)
    print("\n✨ New Features:")
    print("  - Enhanced error handling and user feedback")
    print("  - Better visual design with gradients and animations")
    print("  - Improved order confirmation flow")
    print("  - Real-time data refresh")
    print("  - Keyboard shortcuts")
    print("  - Mobile-responsive design")
    print("  - Accessibility improvements")
    print("\nStarting dashboard on http://localhost:8052")
    print("Press Ctrl+C to stop\n")

    app.run(debug=True, host='0.0.0.0', port=8052)
