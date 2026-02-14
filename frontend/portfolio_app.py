import dash
from dash import dcc, html, Input, Output, State, callback_context
import dash_bootstrap_components as dbc
import requests
import pandas as pd
from datetime import datetime
import plotly.graph_objects as go
from plotly.subplots import make_subplots

# Initialize the Dash app
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.DARKLY],
    suppress_callback_exceptions=True
)

# API Configuration
import os
API_BASE_URL = os.environ.get('API_BASE_URL', 'http://localhost:3000')
API_KEY = os.environ.get('API_KEY', '') or os.environ.get('API_KEYS', '').split(',')[0].strip()

# Headers for API requests
def get_headers():
    return {
        "X-API-Key": API_KEY,
        "Content-Type": "application/json"
    }

# App layout
app.layout = dbc.Container([
    # Header
    dbc.Row([
        dbc.Col([
            html.H1("💰 InvestIQ - Portfolio Manager & Trading Assistant", className="text-center mb-4 mt-4"),
            html.P("Track your positions, log trades, and get actionable insights",
                   className="text-center text-muted mb-4")
        ])
    ]),

    # Account Balance Banner
    dbc.Row([
        dbc.Col([
            html.Div(id='account-balance-banner')
        ])
    ], className="mb-3"),

    # Tabs
    dbc.Tabs([
        # Tab 1: Action Inbox
        dbc.Tab(label="🔔 Action Inbox", tab_id="actions", children=[
            html.Div([
                dbc.Row([
                    dbc.Col([
                        html.Div(id='actions-count', className="mb-3 mt-3")
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        dbc.Button("🔄 Refresh Actions", id='refresh-actions-btn', color="primary", className="mb-3"),
                        html.Div(id='actions-display')
                    ])
                ])
            ])
        ]),

        # Tab 2: Portfolio
        dbc.Tab(label="📊 My Portfolio", tab_id="portfolio", children=[
            html.Div([
                dbc.Row([
                    dbc.Col([
                        html.H3("Portfolio Summary", className="mt-4 mb-3"),
                        html.Div(id='portfolio-summary')
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        dbc.Button("🔄 Refresh Portfolio", id='refresh-portfolio-btn', color="primary", className="mb-3"),
                        dcc.Loading(id="portfolio-loading", children=html.Div(id='portfolio-positions'))
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        html.H4("Add New Position", className="mt-4 mb-3"),
                        dbc.Card([
                            dbc.CardBody([
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Symbol"),
                                        dbc.Input(id='add-symbol', type='text', placeholder='AAPL'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Shares"),
                                        dbc.Input(id='add-shares', type='number', placeholder='10'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Entry Price"),
                                        dbc.Input(id='add-price', type='number', step='0.01', placeholder='150.00'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Entry Date"),
                                        dbc.Input(id='add-date', type='date', value=datetime.now().strftime('%Y-%m-%d')),
                                    ], md=3),
                                    dbc.Col([
                                        dbc.Label("Notes"),
                                        dbc.Input(id='add-notes', type='text', placeholder='Optional notes'),
                                    ], md=3),
                                ]),
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Button("➕ Add Position", id='add-position-btn', color="success", className="mt-3"),
                                        html.Div(id='add-position-result', className="mt-2")
                                    ])
                                ])
                            ])
                        ])
                    ])
                ]),
            ])
        ]),

        # Tab 3: Trade Logger
        dbc.Tab(label="📝 Trade Log", tab_id="trades", children=[
            html.Div([
                dbc.Row([
                    dbc.Col([
                        html.H3("Trade History", className="mt-4 mb-3"),
                        dbc.Button("🔄 Refresh Trades", id='refresh-trades-btn', color="primary", className="mb-3"),
                        html.Div(id='trades-display')
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        html.H4("Log New Trade", className="mt-4 mb-3"),
                        dbc.Card([
                            dbc.CardBody([
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Symbol"),
                                        dbc.Input(id='trade-symbol', type='text', placeholder='AAPL'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Action"),
                                        dcc.Dropdown(
                                            id='trade-action',
                                            options=[
                                                {'label': '🟢 Buy', 'value': 'buy'},
                                                {'label': '🔴 Sell', 'value': 'sell'},
                                            ],
                                            value='buy'
                                        ),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Shares"),
                                        dbc.Input(id='trade-shares', type='number', placeholder='10'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Price"),
                                        dbc.Input(id='trade-price', type='number', step='0.01', placeholder='150.00'),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Date"),
                                        dbc.Input(id='trade-date', type='date', value=datetime.now().strftime('%Y-%m-%d')),
                                    ], md=2),
                                    dbc.Col([
                                        dbc.Label("Commission ($)"),
                                        dbc.Input(id='trade-commission', type='number', step='0.01', value='0', placeholder='0.00'),
                                    ], md=2),
                                ]),
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Notes"),
                                        dbc.Input(id='trade-notes', type='text', placeholder='Optional notes'),
                                    ], md=12),
                                ], className="mt-2"),
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Button("📝 Log Trade", id='log-trade-btn', color="success", className="mt-3"),
                                        html.Div(id='log-trade-result', className="mt-2")
                                    ])
                                ])
                            ])
                        ])
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        html.H4("Performance Metrics", className="mt-4 mb-3"),
                        html.Div(id='performance-metrics')
                    ])
                ])
            ])
        ]),

        # Tab 4: Watchlist
        dbc.Tab(label="👀 Watchlist", tab_id="watchlist", children=[
            html.Div([
                dbc.Row([
                    dbc.Col([
                        html.H3("Watchlist", className="mt-4 mb-3"),
                        dbc.Button("🔄 Refresh Watchlist", id='refresh-watchlist-btn', color="primary", className="mb-3"),
                        html.Div(id='watchlist-display')
                    ])
                ]),
                dbc.Row([
                    dbc.Col([
                        html.H4("Add to Watchlist", className="mt-4 mb-3"),
                        dbc.Card([
                            dbc.CardBody([
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Symbol"),
                                        dbc.Input(id='watch-symbol', type='text', placeholder='AAPL'),
                                    ], md=4),
                                    dbc.Col([
                                        dbc.Label("Notes"),
                                        dbc.Input(id='watch-notes', type='text', placeholder='Why watching this stock?'),
                                    ], md=6),
                                    dbc.Col([
                                        dbc.Button("➕ Add", id='add-watch-btn', color="success", className="mt-4"),
                                    ], md=2),
                                ]),
                                html.Div(id='add-watch-result', className="mt-2")
                            ])
                        ])
                    ])
                ])
            ])
        ]),
    ], id="tabs", active_tab="actions"),

    # Footer
    dbc.Row([
        dbc.Col([
            html.Hr(),
            html.P("⚠️ Disclaimer: Track your trades carefully. This tool does not provide financial advice.",
                   className="text-center text-muted small")
        ])
    ])
], fluid=True)


# Callback for action inbox
@app.callback(
    [Output('actions-count', 'children'),
     Output('actions-display', 'children')],
    [Input('refresh-actions-btn', 'n_clicks'),
     Input('tabs', 'active_tab')]
)
def update_actions(n_clicks, active_tab):
    if active_tab != "actions":
        return "", ""

    try:
        response = requests.get(f'{API_BASE_URL}/api/alerts/actions', headers=get_headers())
        data = response.json()

        if not data.get('success'):
            return "", dbc.Alert(f"Error: {data.get('error')}", color="danger")

        actions = data['data']

        if not actions:
            return html.H5("✅ No urgent actions needed!"), dbc.Alert(
                "All clear! No pending signals or alerts at this time.",
                color="success"
            )

        count_badge = dbc.Badge(f"{len(actions)} Actions Pending", color="danger", className="h5")

        action_cards = []
        for action in actions:
            # Color based on priority
            color_map = {1: 'danger', 2: 'warning', 3: 'info'}
            color = color_map.get(action['priority'], 'secondary')

            # Icon based on type
            icon_map = {
                'buy': '🚀',
                'sell': '📉',
                'stop_loss': '⚠️',
                'take_profit': '🎯',
                'watch': '👀'
            }
            icon = icon_map.get(action['action_type'], '📌')

            action_cards.append(
                dbc.Col([
                    dbc.Card([
                        dbc.CardHeader([
                            html.H5([icon, " ", action['title']], className="mb-0"),
                            dbc.Badge(f"{action['confidence']*100:.0f}% Confidence", color="light", className="float-end")
                        ]),
                        dbc.CardBody([
                            html.P(action['description'], className="mb-2"),
                            html.Hr(),
                            html.Ul([
                                html.Li(f"Signal: {action['signal']}"),
                                html.Li(f"Current Price: ${action['current_price']:.2f}" if action.get('current_price') else "Price: N/A"),
                                html.Li(f"Target: ${action['target_price']:.2f}" if action.get('target_price') else ""),
                                html.Li(f"Stop Loss: ${action['stop_loss_price']:.2f}" if action.get('stop_loss_price') else ""),
                                html.Li(f"{'✅ In your portfolio' if action['in_portfolio'] else '⭕ Not in portfolio'}"),
                            ], className="small"),
                            html.Hr(),
                            dbc.Row([
                                dbc.Col([
                                    dbc.Button("✅ Complete", id={'type': 'complete-action', 'index': action.get('alert_id', 0)},
                                             color="success", size="sm", className="me-2"),
                                    dbc.Button("❌ Ignore", id={'type': 'ignore-action', 'index': action.get('alert_id', 0)},
                                             color="secondary", size="sm"),
                                ])
                            ])
                        ])
                    ], color=color, outline=True, className="mb-3")
                ], md=12, lg=6)
            )

        return count_badge, dbc.Row(action_cards)

    except Exception as e:
        return "", dbc.Alert(f"Error fetching actions: {str(e)}", color="danger")


# Callback for portfolio
@app.callback(
    [Output('portfolio-summary', 'children'),
     Output('portfolio-positions', 'children')],
    [Input('refresh-portfolio-btn', 'n_clicks'),
     Input('tabs', 'active_tab')]
)
def update_portfolio(n_clicks, active_tab):
    if active_tab != "portfolio":
        return "", ""

    try:
        response = requests.get(f'{API_BASE_URL}/api/portfolio', headers=get_headers())
        data = response.json()

        if not data.get('success'):
            return dbc.Alert(f"Error: {data.get('error')}", color="danger"), ""

        summary = data['data']

        # Summary card
        total_pnl_color = "success" if summary['total_pnl'] >= 0 else "danger"
        summary_card = dbc.Card([
            dbc.CardBody([
                dbc.Row([
                    dbc.Col([
                        html.H6("Total Value"),
                        html.H3(f"${summary['total_value']:,.2f}", className="text-success")
                    ], md=3),
                    dbc.Col([
                        html.H6("Total Cost"),
                        html.H3(f"${summary['total_cost']:,.2f}")
                    ], md=3),
                    dbc.Col([
                        html.H6("Total P&L"),
                        html.H3(f"${summary['total_pnl']:,.2f}", className=f"text-{total_pnl_color}")
                    ], md=3),
                    dbc.Col([
                        html.H6("Return %"),
                        html.H3(f"{summary['total_pnl_percent']:.2f}%", className=f"text-{total_pnl_color}")
                    ], md=3),
                ])
            ])
        ], color="dark", className="mb-4")

        # Position cards
        if not summary['positions']:
            positions_display = dbc.Alert("No positions yet. Add your first position below!", color="info")
        else:
            position_rows = []
            for pos in summary['positions']:
                pnl_color = "success" if pos['unrealized_pnl'] >= 0 else "danger"
                pnl_icon = "📈" if pos['unrealized_pnl'] >= 0 else "📉"

                position_rows.append(
                    dbc.Card([
                        dbc.CardBody([
                            dbc.Row([
                                dbc.Col([
                                    html.H4(pos['position']['symbol']),
                                    html.P(f"{pos['position']['shares']} shares", className="text-muted small")
                                ], md=2),
                                dbc.Col([
                                    html.P("Entry Price", className="small text-muted mb-0"),
                                    html.H6(f"${pos['position']['entry_price']:.2f}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Current Price", className="small text-muted mb-0"),
                                    html.H6(f"${pos['current_price']:.2f}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Market Value", className="small text-muted mb-0"),
                                    html.H6(f"${pos['market_value']:,.2f}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Unrealized P&L", className="small text-muted mb-0"),
                                    html.H5(f"{pnl_icon} ${pos['unrealized_pnl']:,.2f}", className=f"text-{pnl_color}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Return %", className="small text-muted mb-0"),
                                    html.H5(f"{pos['unrealized_pnl_percent']:.2f}%", className=f"text-{pnl_color}")
                                ], md=2),
                            ])
                        ])
                    ], className="mb-2")
                )

            positions_display = html.Div(position_rows)

        return summary_card, positions_display

    except Exception as e:
        return dbc.Alert(f"Error fetching portfolio: {str(e)}", color="danger"), ""


# Callback to add position
@app.callback(
    Output('add-position-result', 'children'),
    Input('add-position-btn', 'n_clicks'),
    [State('add-symbol', 'value'),
     State('add-shares', 'value'),
     State('add-price', 'value'),
     State('add-date', 'value'),
     State('add-notes', 'value')]
)
def add_position(n_clicks, symbol, shares, price, date, notes):
    if not n_clicks:
        return ""

    if not all([symbol, shares, price, date]):
        return dbc.Alert("Please fill in all required fields", color="warning")

    try:
        payload = {
            "symbol": symbol.upper(),
            "shares": float(shares),
            "entry_price": float(price),
            "entry_date": date,
            "notes": notes
        }

        response = requests.post(
            f'{API_BASE_URL}/api/portfolio/positions',
            json=payload,
            headers=get_headers()
        )
        data = response.json()

        if data.get('success'):
            return dbc.Alert(f"✅ Added {symbol.upper()} to portfolio!", color="success")
        else:
            return dbc.Alert(f"Error: {data.get('error')}", color="danger")

    except Exception as e:
        return dbc.Alert(f"Error: {str(e)}", color="danger")


# Callback for trades
@app.callback(
    [Output('trades-display', 'children'),
     Output('performance-metrics', 'children')],
    [Input('refresh-trades-btn', 'n_clicks'),
     Input('tabs', 'active_tab')]
)
def update_trades(n_clicks, active_tab):
    if active_tab != "trades":
        return "", ""

    try:
        # Get trades
        trades_response = requests.get(f'{API_BASE_URL}/api/trades', headers=get_headers())
        trades_data = trades_response.json()

        # Get performance
        perf_response = requests.get(f'{API_BASE_URL}/api/trades/performance', headers=get_headers())
        perf_data = perf_response.json()

        if not trades_data.get('success'):
            return dbc.Alert(f"Error: {trades_data.get('error')}", color="danger"), ""

        trades = trades_data['data']

        if not trades:
            trades_display = dbc.Alert("No trades yet. Log your first trade below!", color="info")
        else:
            trade_rows = []
            for trade in trades[:20]:  # Show last 20 trades
                action_badge = dbc.Badge("BUY", color="success") if trade['action'] == 'buy' else dbc.Badge("SELL", color="danger")

                trade_rows.append(
                    dbc.Card([
                        dbc.CardBody([
                            dbc.Row([
                                dbc.Col([
                                    html.H6(trade['symbol']),
                                    action_badge
                                ], md=2),
                                dbc.Col([
                                    html.P("Date", className="small text-muted mb-0"),
                                    html.P(trade['trade_date'])
                                ], md=2),
                                dbc.Col([
                                    html.P("Shares", className="small text-muted mb-0"),
                                    html.P(f"{trade['shares']}")
                                ], md=1),
                                dbc.Col([
                                    html.P("Price", className="small text-muted mb-0"),
                                    html.P(f"${trade['price']:.2f}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Total", className="small text-muted mb-0"),
                                    html.P(f"${trade['shares'] * trade['price']:,.2f}")
                                ], md=2),
                                dbc.Col([
                                    html.P("Commission", className="small text-muted mb-0"),
                                    html.P(f"${trade['commission']:.2f}")
                                ], md=2),
                            ])
                        ])
                    ], className="mb-2")
                )

            trades_display = html.Div(trade_rows)

        # Performance metrics
        if perf_data.get('success'):
            perf = perf_data['data']
            win_rate_color = "success" if perf['win_rate'] > 50 else "warning"

            perf_display = dbc.Card([
                dbc.CardBody([
                    dbc.Row([
                        dbc.Col([
                            html.H6("Total Trades"),
                            html.H4(perf['total_trades'])
                        ], md=3),
                        dbc.Col([
                            html.H6("Win Rate"),
                            html.H4(f"{perf['win_rate']:.1f}%", className=f"text-{win_rate_color}")
                        ], md=3),
                        dbc.Col([
                            html.H6("Total Realized P&L"),
                            html.H4(f"${perf['total_realized_pnl']:,.2f}",
                                   className="text-success" if perf['total_realized_pnl'] >= 0 else "text-danger")
                        ], md=3),
                        dbc.Col([
                            html.H6("Avg Win / Avg Loss"),
                            html.H6(f"${perf['average_win']:.2f} / ${perf['average_loss']:.2f}")
                        ], md=3),
                    ])
                ])
            ], color="dark")
        else:
            perf_display = ""

        return trades_display, perf_display

    except Exception as e:
        return dbc.Alert(f"Error fetching trades: {str(e)}", color="danger"), ""


# Callback to log trade
@app.callback(
    Output('log-trade-result', 'children'),
    Input('log-trade-btn', 'n_clicks'),
    [State('trade-symbol', 'value'),
     State('trade-action', 'value'),
     State('trade-shares', 'value'),
     State('trade-price', 'value'),
     State('trade-date', 'value'),
     State('trade-commission', 'value'),
     State('trade-notes', 'value')]
)
def log_trade(n_clicks, symbol, action, shares, price, date, commission, notes):
    if not n_clicks:
        return ""

    if not all([symbol, action, shares, price, date]):
        return dbc.Alert("Please fill in all required fields", color="warning")

    try:
        payload = {
            "symbol": symbol.upper(),
            "action": action,
            "shares": float(shares),
            "price": float(price),
            "trade_date": date,
            "commission": float(commission) if commission else 0.0,
            "notes": notes
        }

        response = requests.post(
            f'{API_BASE_URL}/api/trades',
            json=payload,
            headers=get_headers()
        )
        data = response.json()

        if data.get('success'):
            return dbc.Alert(f"✅ Trade logged: {action.upper()} {shares} {symbol.upper()}", color="success")
        else:
            return dbc.Alert(f"Error: {data.get('error')}", color="danger")

    except Exception as e:
        return dbc.Alert(f"Error: {str(e)}", color="danger")


# Callback for watchlist
@app.callback(
    Output('watchlist-display', 'children'),
    [Input('refresh-watchlist-btn', 'n_clicks'),
     Input('tabs', 'active_tab')]
)
def update_watchlist(n_clicks, active_tab):
    if active_tab != "watchlist":
        return ""

    try:
        response = requests.get(f'{API_BASE_URL}/api/watchlist', headers=get_headers())
        data = response.json()

        if not data.get('success'):
            return dbc.Alert(f"Error: {data.get('error')}", color="danger")

        watchlist = data['data']

        if not watchlist:
            return dbc.Alert("Watchlist is empty. Add stocks you want to monitor!", color="info")

        watch_rows = []
        for item in watchlist:
            watch_rows.append(
                dbc.Card([
                    dbc.CardBody([
                        dbc.Row([
                            dbc.Col([
                                html.H5(item['symbol'])
                            ], md=2),
                            dbc.Col([
                                html.P(item['notes'] or "No notes", className="text-muted")
                            ], md=8),
                            dbc.Col([
                                dbc.Button("Remove", id={'type': 'remove-watch', 'index': item['symbol']},
                                         color="danger", size="sm")
                            ], md=2),
                        ])
                    ])
                ], className="mb-2")
            )

        return html.Div(watch_rows)

    except Exception as e:
        return dbc.Alert(f"Error fetching watchlist: {str(e)}", color="danger")


# Callback to add to watchlist
@app.callback(
    Output('add-watch-result', 'children'),
    Input('add-watch-btn', 'n_clicks'),
    [State('watch-symbol', 'value'),
     State('watch-notes', 'value')]
)
def add_to_watchlist(n_clicks, symbol, notes):
    if not n_clicks or not symbol:
        return ""

    try:
        payload = {
            "symbol": symbol.upper(),
            "notes": notes
        }

        response = requests.post(
            f'{API_BASE_URL}/api/watchlist',
            json=payload,
            headers=get_headers()
        )
        data = response.json()

        if data.get('success'):
            return dbc.Alert(f"✅ Added {symbol.upper()} to watchlist!", color="success")
        else:
            return dbc.Alert(f"Error: {data.get('error')}", color="danger")

    except Exception as e:
        return dbc.Alert(f"Error: {str(e)}", color="danger")


if __name__ == '__main__':
    print("🚀 Starting InvestIQ Portfolio Manager...")
    print("📊 Dashboard will be available at: http://localhost:8052")
    print("⚠️  Make sure the API server is running on http://localhost:3000")
    print("⚠️  Update API_KEY in this file with your actual API key!")
    app.run_server(debug=True, host='0.0.0.0', port=8052)
