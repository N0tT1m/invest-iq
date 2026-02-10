import dash
from dash import dcc, html, Input, Output, State, ALL, ctx
import dash.dependencies
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import requests
import pandas as pd
from datetime import datetime, timedelta
import dash_bootstrap_components as dbc
import json

# Initialize the Dash app with a modern theme
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.DARKLY, dbc.icons.FONT_AWESOME],
    suppress_callback_exceptions=True,
    title="InvestIQ - Smart Stock Analysis"
)

# API Configuration
API_BASE_URL = "http://localhost:3000"

# Store for user preferences
WATCHLIST_STORE = []
RECENT_SYMBOLS = []

# Glossary for tooltips
GLOSSARY = {
    'RSI': 'Relative Strength Index - Momentum indicator (0-100). Below 30 = oversold, above 70 = overbought',
    'MACD': 'Moving Average Convergence Divergence - Trend following indicator. Crossovers signal potential buy/sell',
    'Sharpe Ratio': 'Risk-adjusted return metric. Higher is better. Above 1.0 is good, above 2.0 is very good',
    'Beta': 'Market sensitivity. 1.0 = moves with market, >1.0 = more volatile, <1.0 = less volatile',
    'P/E Ratio': 'Price-to-Earnings ratio. Lower may indicate undervaluation (compare to industry average)',
    'ROE': 'Return on Equity - Profitability metric. Higher percentage indicates better returns for shareholders',
    'Max Drawdown': 'Largest peak-to-trough decline. Shows worst-case historical loss',
    'VaR': 'Value at Risk - Maximum expected loss at 95% confidence level',
    'Volatility': 'Price variation over time. Higher = more risky',
    'Bollinger Bands': 'Volatility bands. Price touching upper = overbought, lower = oversold'
}

# Skeleton loader component
def create_skeleton_loader(height=200):
    return dbc.Card([
        dbc.CardBody([
            html.Div([
                html.Div(className="skeleton-line", style={'width': '60%', 'height': '20px', 'marginBottom': '10px'}),
                html.Div(className="skeleton-line", style={'width': '80%', 'height': '15px', 'marginBottom': '10px'}),
                html.Div(className="skeleton-line", style={'width': '40%', 'height': '15px', 'marginBottom': '20px'}),
                html.Div(className="skeleton-box", style={'width': '100%', 'height': f'{height}px'}),
            ])
        ])
    ], className="skeleton-card")

# Help tooltip component
def create_help_tooltip(term_id, term):
    if term not in GLOSSARY:
        return html.Span(term)

    return html.Span([
        term,
        html.I(
            className="fas fa-question-circle ms-1",
            id=f"tooltip-{term_id}",
            style={'cursor': 'pointer', 'fontSize': '0.8em', 'color': '#667eea'}
        ),
        dbc.Tooltip(
            GLOSSARY[term],
            target=f"tooltip-{term_id}",
            placement="top"
        )
    ])

# App layout
app.layout = dbc.Container([
    # Storage components
    dcc.Store(id='watchlist-store', data=[]),
    dcc.Store(id='recent-symbols-store', data=[]),
    dcc.Store(id='user-preferences', data={'theme': 'dark', 'autoRefresh': False, 'refreshInterval': 30}),
    dcc.Store(id='comparison-symbols', data=[]),
    dcc.Store(id='onboarding-complete', data=False),

    # Interval for auto-refresh
    dcc.Interval(id='auto-refresh-interval', interval=30000, n_intervals=0, disabled=True),

    # Keyboard shortcut listener
    html.Div(id='keyboard-listener', tabIndex=0, style={'outline': 'none'}),

    # Welcome/Onboarding Modal
    dbc.Modal([
        dbc.ModalHeader(dbc.ModalTitle("Welcome to InvestIQ! 👋")),
        dbc.ModalBody([
            html.H5("Your AI-Powered Stock Analysis Platform", className="mb-3"),
            html.P("InvestIQ combines multiple analysis methods to give you comprehensive insights:"),
            dbc.Row([
                dbc.Col([
                    html.Div([
                        html.I(className="fas fa-chart-line fa-2x text-primary mb-2"),
                        html.H6("Technical Analysis"),
                        html.P("Chart patterns, indicators like RSI, MACD, and price action", className="small text-muted")
                    ], className="text-center mb-3")
                ], md=3),
                dbc.Col([
                    html.Div([
                        html.I(className="fas fa-building fa-2x text-success mb-2"),
                        html.H6("Fundamental Analysis"),
                        html.P("Financial metrics, P/E ratio, profitability, and company health", className="small text-muted")
                    ], className="text-center mb-3")
                ], md=3),
                dbc.Col([
                    html.Div([
                        html.I(className="fas fa-calculator fa-2x text-warning mb-2"),
                        html.H6("Quantitative Analysis"),
                        html.P("Risk metrics, Sharpe ratio, volatility, and statistical analysis", className="small text-muted")
                    ], className="text-center mb-3")
                ], md=3),
                dbc.Col([
                    html.Div([
                        html.I(className="fas fa-newspaper fa-2x text-info mb-2"),
                        html.H6("Sentiment Analysis"),
                        html.P("News sentiment, market perception, and social signals", className="small text-muted")
                    ], className="text-center mb-3")
                ], md=3),
            ]),
            html.Hr(),
            html.H6("Quick Tips:", className="mt-3"),
            html.Ul([
                html.Li("Press '/' to quickly focus on the search box"),
                html.Li("Use the watchlist to track your favorite stocks"),
                html.Li("Click the help icons (?) for explanations of technical terms"),
                html.Li("Compare multiple stocks side-by-side"),
                html.Li("Export analysis results for offline review"),
            ]),
            html.Hr(),
            dbc.Alert([
                html.I(className="fas fa-exclamation-triangle me-2"),
                "Disclaimer: This tool is for educational purposes only. Not financial advice. Always consult a professional."
            ], color="warning", className="mt-3")
        ]),
        dbc.ModalFooter([
            dbc.Button("Take a Quick Tour", id="start-tour-btn", color="info", className="me-2"),
            dbc.Button("Get Started", id="close-welcome-btn", color="primary")
        ])
    ], id="welcome-modal", is_open=True, size="xl", backdrop="static"),

    # Header with improved navigation
    dbc.Navbar([
        dbc.Container([
            dbc.Row([
                dbc.Col([
                    dbc.NavbarBrand([
                        html.I(className="fas fa-chart-line me-2"),
                        "InvestIQ"
                    ], className="ms-2", style={'fontSize': '1.5rem', 'fontWeight': '700'}),
                ], width="auto"),
                dbc.Col([
                    dbc.Nav([
                        dbc.NavItem(dbc.NavLink([html.I(className="fas fa-home me-1"), "Dashboard"], href="#", id="nav-dashboard", active=True)),
                        dbc.NavItem(dbc.NavLink([html.I(className="fas fa-star me-1"), "Watchlist"], href="#", id="nav-watchlist")),
                        dbc.NavItem(dbc.NavLink([html.I(className="fas fa-history me-1"), "Recent"], href="#", id="nav-recent")),
                        dbc.NavItem(dbc.NavLink([html.I(className="fas fa-question-circle me-1"), "Help"], href="#", id="nav-help")),
                    ], navbar=True),
                ], width="auto"),
                dbc.Col([
                    dbc.ButtonGroup([
                        dbc.Button(html.I(className="fas fa-moon"), id="theme-toggle", color="secondary", size="sm", outline=True),
                        dbc.Button(html.I(className="fas fa-cog"), id="settings-btn", color="secondary", size="sm", outline=True),
                    ], size="sm", className="ms-auto")
                ], width="auto", className="ms-auto"),
            ], className="w-100 align-items-center", justify="between"),
        ], fluid=True)
    ], color="dark", dark=True, className="mb-4", sticky="top"),

    # Search and Controls - Enhanced
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardBody([
                    dbc.Row([
                        dbc.Col([
                            dbc.Label([
                                "Stock Symbol ",
                                html.I(className="fas fa-keyboard text-muted", style={'fontSize': '0.8em'}),
                                html.Small(" (Press '/' to focus)", className="text-muted")
                            ]),
                            dcc.Dropdown(
                                id='symbol-input',
                                options=[],
                                value='AAPL',
                                placeholder='Search symbols... (e.g., AAPL, MSFT, TSLA)',
                                className="mb-2",
                                searchable=True,
                                clearable=True,
                            ),
                        ], md=3),
                        dbc.Col([
                            dbc.Label("Timeframe"),
                            dcc.Dropdown(
                                id='timeframe-dropdown',
                                options=[
                                    {'label': '📊 1 Minute', 'value': '1m'},
                                    {'label': '📊 5 Minutes', 'value': '5m'},
                                    {'label': '📊 15 Minutes', 'value': '15m'},
                                    {'label': '📊 1 Hour', 'value': '1h'},
                                    {'label': '📊 1 Day', 'value': '1d'},
                                    {'label': '📊 1 Week', 'value': '1w'},
                                ],
                                value='1d',
                                className="mb-2"
                            ),
                        ], md=2),
                        dbc.Col([
                            dbc.Label("Period"),
                            dbc.ButtonGroup([
                                dbc.Button("1W", id={'type': 'period-btn', 'period': 7}, size="sm", outline=True, color="primary"),
                                dbc.Button("1M", id={'type': 'period-btn', 'period': 30}, size="sm", outline=True, color="primary"),
                                dbc.Button("3M", id={'type': 'period-btn', 'period': 90}, size="sm", outline=True, color="primary", active=True),
                                dbc.Button("6M", id={'type': 'period-btn', 'period': 180}, size="sm", outline=True, color="primary"),
                                dbc.Button("1Y", id={'type': 'period-btn', 'period': 365}, size="sm", outline=True, color="primary"),
                            ], className="w-100"),
                            dcc.Store(id='days-store', data=90),
                        ], md=3),
                        dbc.Col([
                            dbc.Label("Actions"),
                            html.Br(),
                            dbc.ButtonGroup([
                                dbc.Button([
                                    html.I(className="fas fa-search me-1"),
                                    "Analyze"
                                ], id='analyze-button', color="primary", className="me-1", n_clicks=0),
                                dbc.Button(html.I(className="fas fa-sync-alt"), id='refresh-button', color="secondary", outline=True, n_clicks=0),
                                dbc.Button(html.I(className="fas fa-star"), id='add-watchlist-btn', color="warning", outline=True),
                                dbc.Button(html.I(className="fas fa-download"), id='export-btn', color="info", outline=True),
                            ], className="w-100"),
                        ], md=2),
                        dbc.Col([
                            dbc.Label("Compare"),
                            html.Br(),
                            dbc.Button([
                                html.I(className="fas fa-plus me-1"),
                                "Add to Compare"
                            ], id='add-compare-btn', color="success", outline=True, size="sm", className="w-100"),
                        ], md=2),
                    ], className="align-items-end")
                ])
            ], className="mb-3")
        ])
    ]),

    # Comparison mode banner
    html.Div(id='comparison-banner'),

    # Last updated & auto-refresh controls
    dbc.Row([
        dbc.Col([
            html.Div([
                html.Small([
                    html.I(className="fas fa-clock me-1"),
                    html.Span("Last updated: ", className="text-muted"),
                    html.Span(id='last-updated', className="text-info"),
                    html.Span(" | ", className="text-muted mx-2"),
                    dbc.Checklist(
                        options=[{"label": "Auto-refresh (30s)", "value": 1}],
                        value=[],
                        id="auto-refresh-toggle",
                        switch=True,
                        inline=True,
                        className="d-inline"
                    )
                ])
            ], className="text-end")
        ])
    ], className="mb-3"),

    # Loading indicator
    dbc.Row([
        dbc.Col([
            dcc.Loading(
                id="loading",
                type="default",
                children=html.Div(id="loading-output"),
                className="loading-custom"
            )
        ])
    ]),

    # Overall Signal Card
    dbc.Row([
        dbc.Col([
            html.Div(id='overall-signal-card')
        ])
    ], className="mb-4"),

    # Main content tabs
    dbc.Tabs([
        dbc.Tab(label="📊 Charts & Analysis", tab_id="tab-charts", active_tab_class_name="fw-bold"),
        dbc.Tab(label="📈 Technical Deep Dive", tab_id="tab-technical", active_tab_class_name="fw-bold"),
        dbc.Tab(label="💼 Fundamental Analysis", tab_id="tab-fundamental", active_tab_class_name="fw-bold"),
        dbc.Tab(label="🔢 Risk & Quant", tab_id="tab-quant", active_tab_class_name="fw-bold"),
        dbc.Tab(label="📰 News & Sentiment", tab_id="tab-sentiment", active_tab_class_name="fw-bold"),
        dbc.Tab(label="🔍 Compare Stocks", tab_id="tab-compare", active_tab_class_name="fw-bold"),
    ], id="main-tabs", active_tab="tab-charts", className="mb-4"),

    html.Div(id='tab-content'),

    # Watchlist Sidebar (collapsible)
    dbc.Offcanvas([
        html.H4("⭐ Watchlist"),
        html.Hr(),
        html.Div(id='watchlist-content'),
        html.Hr(),
        dbc.Button("Clear All", id="clear-watchlist-btn", color="danger", size="sm", outline=True, className="w-100")
    ], id="watchlist-offcanvas", title="Your Watchlist", is_open=False, placement="end"),

    # Recent Sidebar
    dbc.Offcanvas([
        html.H4("🕐 Recently Viewed"),
        html.Hr(),
        html.Div(id='recent-content')
    ], id="recent-offcanvas", title="Recent Stocks", is_open=False, placement="end"),

    # Settings Modal
    dbc.Modal([
        dbc.ModalHeader(dbc.ModalTitle("⚙️ Settings")),
        dbc.ModalBody([
            html.H6("Display Preferences"),
            dbc.Checklist(
                options=[
                    {"label": "Dark Mode", "value": "dark"},
                    {"label": "Show Advanced Metrics", "value": "advanced"},
                    {"label": "Compact View", "value": "compact"},
                ],
                value=["dark"],
                id="display-settings",
            ),
            html.Hr(),
            html.H6("Refresh Settings"),
            dbc.Label("Auto-refresh interval (seconds)"),
            dbc.Input(type="number", value=30, min=10, max=300, step=10, id="refresh-interval-input"),
            html.Hr(),
            html.H6("Chart Settings"),
            dbc.Checklist(
                options=[
                    {"label": "Show Volume", "value": "volume"},
                    {"label": "Show Bollinger Bands", "value": "bb"},
                    {"label": "Show Moving Averages", "value": "ma"},
                ],
                value=["volume", "bb", "ma"],
                id="chart-settings",
            ),
        ]),
        dbc.ModalFooter([
            dbc.Button("Reset to Defaults", id="reset-settings-btn", color="secondary", className="me-auto"),
            dbc.Button("Save", id="save-settings-btn", color="primary")
        ])
    ], id="settings-modal", is_open=False),

    # Help Modal
    dbc.Modal([
        dbc.ModalHeader(dbc.ModalTitle("❓ Help & Keyboard Shortcuts")),
        dbc.ModalBody([
            html.H5("Keyboard Shortcuts"),
            dbc.Table([
                html.Tbody([
                    html.Tr([html.Td(html.Kbd("/")), html.Td("Focus search box")]),
                    html.Tr([html.Td(html.Kbd("Enter")), html.Td("Analyze stock")]),
                    html.Tr([html.Td(html.Kbd("R")), html.Td("Refresh data")]),
                    html.Tr([html.Td(html.Kbd("W")), html.Td("Toggle watchlist")]),
                    html.Tr([html.Td(html.Kbd("H")), html.Td("Show help")]),
                    html.Tr([html.Td(html.Kbd("E")), html.Td("Export data")]),
                    html.Tr([html.Td(html.Kbd("Esc")), html.Td("Close modals")]),
                ])
            ], bordered=True, size="sm"),
            html.Hr(),
            html.H5("Understanding the Analysis"),
            dbc.Accordion([
                dbc.AccordionItem([
                    html.P("Technical analysis uses price charts and indicators to predict future movements:"),
                    html.Ul([
                        html.Li("RSI: Shows if a stock is overbought (>70) or oversold (<30)"),
                        html.Li("MACD: Identifies trend changes through moving average crossovers"),
                        html.Li("Bollinger Bands: Show price volatility and potential breakouts"),
                    ])
                ], title="Technical Analysis"),
                dbc.AccordionItem([
                    html.P("Fundamental analysis evaluates company financial health:"),
                    html.Ul([
                        html.Li("P/E Ratio: Lower values may indicate undervaluation"),
                        html.Li("ROE: Higher percentages show better returns for shareholders"),
                        html.Li("Debt/Equity: Lower is generally better (less financial risk)"),
                    ])
                ], title="Fundamental Analysis"),
                dbc.AccordionItem([
                    html.P("Quantitative analysis measures risk and statistical performance:"),
                    html.Ul([
                        html.Li("Sharpe Ratio: Risk-adjusted returns (>1 is good, >2 is excellent)"),
                        html.Li("Volatility: Price fluctuation (higher = more risky)"),
                        html.Li("Max Drawdown: Worst historical loss from peak to trough"),
                    ])
                ], title="Quantitative Analysis"),
            ], start_collapsed=True),
        ]),
        dbc.ModalFooter([
            dbc.Button("Close", id="close-help-btn", color="primary")
        ])
    ], id="help-modal", is_open=False, size="lg"),

    # Footer
    dbc.Row([
        dbc.Col([
            html.Hr(),
            html.Div([
                html.P([
                    html.I(className="fas fa-exclamation-triangle me-2"),
                    "Disclaimer: This tool is for educational purposes only. Not financial advice. Always consult a professional."
                ], className="text-center text-muted small mb-2"),
                html.P([
                    "Made with ",
                    html.I(className="fas fa-heart text-danger"),
                    " by InvestIQ Team | ",
                    html.A("Documentation", href="#", className="text-info"),
                    " | ",
                    html.A("Report Issue", href="#", className="text-info"),
                ], className="text-center text-muted small")
            ])
        ])
    ])
], fluid=True, className="px-4 py-3")


# Callback for onboarding
@app.callback(
    Output('welcome-modal', 'is_open'),
    Output('onboarding-complete', 'data'),
    [Input('close-welcome-btn', 'n_clicks'),
     Input('start-tour-btn', 'n_clicks')],
    [State('onboarding-complete', 'data')],
    prevent_initial_call=True
)
def handle_onboarding(close_clicks, tour_clicks, onboarding_done):
    if ctx.triggered_id == 'close-welcome-btn':
        return False, True
    elif ctx.triggered_id == 'start-tour-btn':
        # In a real implementation, this would start a step-by-step tour
        return False, True
    return not onboarding_done, onboarding_done


# Callback for navigation
@app.callback(
    [Output('watchlist-offcanvas', 'is_open'),
     Output('recent-offcanvas', 'is_open'),
     Output('help-modal', 'is_open')],
    [Input('nav-watchlist', 'n_clicks'),
     Input('nav-recent', 'n_clicks'),
     Input('nav-help', 'n_clicks'),
     Input('close-help-btn', 'n_clicks')],
    [State('watchlist-offcanvas', 'is_open'),
     State('recent-offcanvas', 'is_open'),
     State('help-modal', 'is_open')],
    prevent_initial_call=True
)
def toggle_navigation(watch_clicks, recent_clicks, help_clicks, close_help,
                      watch_open, recent_open, help_open):
    if ctx.triggered_id == 'nav-watchlist':
        return not watch_open, False, False
    elif ctx.triggered_id == 'nav-recent':
        return False, not recent_open, False
    elif ctx.triggered_id == 'nav-help':
        return False, False, not help_open
    elif ctx.triggered_id == 'close-help-btn':
        return False, False, False
    return watch_open, recent_open, help_open


# Callback for settings
@app.callback(
    Output('settings-modal', 'is_open'),
    [Input('settings-btn', 'n_clicks'),
     Input('save-settings-btn', 'n_clicks')],
    [State('settings-modal', 'is_open')],
    prevent_initial_call=True
)
def toggle_settings(settings_clicks, save_clicks, is_open):
    return not is_open


# Callback for period buttons
@app.callback(
    Output('days-store', 'data'),
    Input({'type': 'period-btn', 'period': ALL}, 'n_clicks'),
    prevent_initial_call=True
)
def update_period(n_clicks):
    if not any(n_clicks):
        return 90

    triggered = ctx.triggered_id
    if triggered and isinstance(triggered, dict):
        return triggered['period']
    return 90


# Callback for auto-refresh
@app.callback(
    Output('auto-refresh-interval', 'disabled'),
    Input('auto-refresh-toggle', 'value')
)
def toggle_auto_refresh(value):
    return len(value) == 0


# Callback to update last updated time
@app.callback(
    Output('last-updated', 'children'),
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks'),
     Input('auto-refresh-interval', 'n_intervals')]
)
def update_timestamp(analyze, refresh, intervals):
    return datetime.now().strftime("%I:%M:%S %p")


# Callback for watchlist management
@app.callback(
    [Output('watchlist-store', 'data'),
     Output('add-watchlist-btn', 'color')],
    [Input('add-watchlist-btn', 'n_clicks'),
     Input('clear-watchlist-btn', 'n_clicks')],
    [State('symbol-input', 'value'),
     State('watchlist-store', 'data')],
    prevent_initial_call=True
)
def manage_watchlist(add_clicks, clear_clicks, symbol, watchlist):
    if ctx.triggered_id == 'clear-watchlist-btn':
        return [], 'warning'

    if ctx.triggered_id == 'add-watchlist-btn' and symbol:
        if symbol in watchlist:
            # Remove from watchlist
            watchlist.remove(symbol)
            return watchlist, 'warning'
        else:
            # Add to watchlist
            watchlist.append(symbol)
            return watchlist, 'success'

    return watchlist, 'warning' if symbol not in watchlist else 'success'


# Callback to display watchlist
@app.callback(
    Output('watchlist-content', 'children'),
    Input('watchlist-store', 'data')
)
def display_watchlist(watchlist):
    if not watchlist:
        return dbc.Alert("No stocks in your watchlist. Click the star button to add stocks!", color="info", className="text-center")

    items = []
    for symbol in watchlist:
        items.append(
            dbc.ListGroupItem([
                dbc.Row([
                    dbc.Col(html.Strong(symbol), width=4),
                    dbc.Col([
                        dbc.Button("View", size="sm", color="primary", outline=True, id={'type': 'view-watchlist', 'symbol': symbol}, className="me-1"),
                        dbc.Button("Remove", size="sm", color="danger", outline=True, id={'type': 'remove-watchlist', 'symbol': symbol}),
                    ], width=8, className="text-end")
                ])
            ])
        )

    return dbc.ListGroup(items)


# Callback for comparison mode
@app.callback(
    [Output('comparison-symbols', 'data'),
     Output('comparison-banner', 'children')],
    [Input('add-compare-btn', 'n_clicks')],
    [State('symbol-input', 'value'),
     State('comparison-symbols', 'data')],
    prevent_initial_call=True
)
def manage_comparison(n_clicks, symbol, comparison):
    if symbol and symbol not in comparison:
        comparison.append(symbol)

    if len(comparison) == 0:
        return comparison, None

    banner = dbc.Alert([
        html.Strong("Comparison Mode: "),
        html.Span(f"{len(comparison)} stocks - "),
        *[dbc.Badge(sym, color="info", className="me-1") for sym in comparison],
        dbc.Button("Clear All", size="sm", color="danger", outline=True, className="ms-2", id="clear-comparison-btn")
    ], color="info", className="d-flex align-items-center")

    return comparison, banner


# Main analysis callback
@app.callback(
    Output('tab-content', 'children'),
    [Input('main-tabs', 'active_tab'),
     Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks'),
     Input('auto-refresh-interval', 'n_intervals')],
    [State('symbol-input', 'value'),
     State('timeframe-dropdown', 'value'),
     State('days-store', 'data'),
     State('comparison-symbols', 'data'),
     State('chart-settings', 'value')]
)
def render_tab_content(active_tab, analyze_clicks, refresh_clicks, intervals,
                       symbol, timeframe, days, comparison_symbols, chart_settings):
    if not symbol:
        return dbc.Alert("Please enter a stock symbol to analyze", color="warning", className="text-center")

    # This is a placeholder - actual implementation would fetch and display data
    if active_tab == "tab-charts":
        return create_charts_tab(symbol, timeframe, days, chart_settings)
    elif active_tab == "tab-technical":
        return create_technical_tab(symbol)
    elif active_tab == "tab-fundamental":
        return create_fundamental_tab(symbol)
    elif active_tab == "tab-quant":
        return create_quant_tab(symbol)
    elif active_tab == "tab-sentiment":
        return create_sentiment_tab(symbol)
    elif active_tab == "tab-compare":
        return create_comparison_tab(comparison_symbols, timeframe, days)

    return html.Div("Content loading...")


def create_charts_tab(symbol, timeframe, days, chart_settings):
    """Create the main charts tab with enhanced features"""
    return html.Div([
        dbc.Row([
            dbc.Col([
                dbc.Card([
                    dbc.CardHeader([
                        html.H5("📊 Price Chart", className="mb-0 d-inline"),
                        dbc.ButtonGroup([
                            dbc.Button(html.I(className="fas fa-expand"), size="sm", color="secondary", outline=True),
                            dbc.Button(html.I(className="fas fa-cog"), size="sm", color="secondary", outline=True),
                        ], size="sm", className="float-end")
                    ]),
                    dbc.CardBody([
                        dcc.Graph(id='main-chart-enhanced', config={'displayModeBar': True, 'displaylogo': False})
                    ])
                ])
            ])
        ], className="mb-3"),

        dbc.Row([
            dbc.Col([
                dbc.Card([
                    dbc.CardHeader(html.H6([create_help_tooltip("rsi-header", "RSI"), " Indicator"])),
                    dbc.CardBody([
                        dcc.Graph(id='rsi-chart-enhanced', config={'displayModeBar': False})
                    ])
                ])
            ], md=6),
            dbc.Col([
                dbc.Card([
                    dbc.CardHeader(html.H6([create_help_tooltip("macd-header", "MACD"), " Indicator"])),
                    dbc.CardBody([
                        dcc.Graph(id='macd-chart-enhanced', config={'displayModeBar': False})
                    ])
                ])
            ], md=6),
        ])
    ])


def create_technical_tab(symbol):
    """Create technical analysis deep dive tab"""
    return html.Div([
        dbc.Alert("Technical deep dive for " + symbol, color="info"),
        # Add detailed technical analysis components here
    ])


def create_fundamental_tab(symbol):
    """Create fundamental analysis tab"""
    return html.Div([
        dbc.Alert("Fundamental analysis for " + symbol, color="info"),
        # Add fundamental analysis components here
    ])


def create_quant_tab(symbol):
    """Create quantitative analysis tab"""
    return html.Div([
        dbc.Alert("Quantitative analysis for " + symbol, color="info"),
        # Add quant analysis components here
    ])


def create_sentiment_tab(symbol):
    """Create sentiment analysis tab"""
    return html.Div([
        dbc.Alert("Sentiment analysis for " + symbol, color="info"),
        # Add sentiment analysis components here
    ])


def create_comparison_tab(symbols, timeframe, days):
    """Create stock comparison tab"""
    if not symbols or len(symbols) < 2:
        return dbc.Alert("Add at least 2 stocks to compare", color="warning", className="text-center")

    return html.Div([
        dbc.Alert(f"Comparing {len(symbols)} stocks: {', '.join(symbols)}", color="info"),
        # Add comparison charts and metrics here
    ])


if __name__ == '__main__':
    print("🚀 Starting InvestIQ Enhanced Dashboard...")
    print("📊 Dashboard will be available at: http://localhost:8050")
    print("⚠️  Make sure the API server is running on http://localhost:3000")
    print("\n💡 New Features:")
    print("  - Welcome tour and onboarding")
    print("  - Keyboard shortcuts (press 'H' for help)")
    print("  - Watchlist and recent stocks")
    print("  - Stock comparison mode")
    print("  - Dark/Light theme toggle")
    print("  - Auto-refresh controls")
    print("  - Export functionality")
    print("  - Improved accessibility\n")

    app.run_server(debug=True, host='0.0.0.0', port=8050)
