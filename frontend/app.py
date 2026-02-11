import os
from pathlib import Path
from dotenv import load_dotenv

# Load .env from project root (parent of frontend/)
load_dotenv(Path(__file__).resolve().parent.parent / ".env")

import dash
from dash import dcc, html, Input, Output, State
import dash.dependencies
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import requests
import pandas as pd
from datetime import datetime
import dash_bootstrap_components as dbc
import diskcache
from concurrent.futures import ThreadPoolExecutor, as_completed

from components.risk_radar import RiskRadarComponent, create_risk_radar_chart, create_risk_breakdown_bars
from components.confidence_gauge import ConfidenceGaugeComponent, create_confidence_gauge
from components.sentiment_velocity import SentimentVelocityComponent, create_velocity_gauge, create_sentiment_history_chart
from components.earnings_panel import EarningsPanelComponent
from components.dividend_panel import DividendPanelComponent
from components.options_flow import OptionsFlowComponent
from components.short_interest import ShortInterestComponent
from components.insider_activity import InsiderActivityComponent
from components.correlation_matrix import CorrelationMatrixComponent
from components.social_sentiment import SocialSentimentComponent
from components.macro_overlay import MacroOverlayComponent
from components.paper_trading import PaperTradingComponent
from components.live_trading import LiveTradingComponent
from components.portfolio_dashboard import PortfolioDashboardComponent
from components.backtest_panel import BacktestPanelComponent
from components.alpha_decay import AlphaDecayComponent
from components.flow_map import FlowMapComponent
from components.smart_watchlist import SmartWatchlistComponent
from components.tax_dashboard import TaxDashboardComponent
from components.agent_trades import AgentTradesComponent
from components.symbol_search import SymbolSearchComponent

# Initialize the Dash app with a modern theme
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.DARKLY],
    suppress_callback_exceptions=True,
)

# API Configuration
API_BASE_URL = os.getenv("API_BASE_URL", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "") or os.getenv("API_KEYS", "").split(",")[0].strip()

# Shared frontend cache (diskcache so background processes can read/write)
_frontend_cache = diskcache.Cache(os.path.join(os.path.dirname(__file__), ".cache", "app"))
_FRONTEND_CACHE_TTL = 300  # 5 minutes


def _cache_get(key):
    """Get a value from frontend cache if not expired"""
    return _frontend_cache.get(key)


def _cache_set(key, data):
    """Store a value in the frontend cache"""
    _frontend_cache.set(key, data, expire=_FRONTEND_CACHE_TTL)

# Production safety: require API_KEY if PRODUCTION=true
_is_production = os.getenv("PRODUCTION", "").lower() in ("true", "1", "yes")
if _is_production and not API_KEY:
    raise RuntimeError("PRODUCTION=true but API_KEY is not set. Refusing to start.")

# Warn if API key is not set
if not API_KEY:
    import warnings
    warnings.warn("API_KEY not set. Set API_KEY in .env file.", stacklevel=2)

# Headers for API requests
def get_headers():
    return {
        "X-API-Key": API_KEY,
        "Content-Type": "application/json"
    }

# Sector peer groups for comparison (mirrors backend tax-optimizer mappings)
SECTOR_PEERS = {
    "Technology": ["AAPL", "MSFT", "NVDA", "INTC", "AMD", "CSCO", "ADBE", "CRM", "ORCL", "IBM", "QCOM", "NOW"],
    "Communication": ["GOOGL", "META", "NFLX", "DIS", "VZ"],
    "Consumer Discretionary": ["AMZN", "TSLA", "HD", "NKE", "MCD", "COST"],
    "Consumer Staples": ["PG", "PEP", "KO", "WMT"],
    "Financial": ["JPM", "V", "MA", "BAC", "GS", "MS", "BRK.B"],
    "Healthcare": ["JNJ", "UNH", "PFE", "MRK", "ABBV", "TMO"],
    "Energy": ["XOM", "CVX"],
    "Industrial": ["HON", "UPS", "CAT"],
}

# Reverse lookup: symbol -> sector
_SYMBOL_TO_SECTOR = {}
for _sector, _symbols in SECTOR_PEERS.items():
    for _sym in _symbols:
        _SYMBOL_TO_SECTOR[_sym] = _sector


def get_peers(symbol, limit=4):
    """Get sector peers for a symbol, excluding the symbol itself"""
    sector = _SYMBOL_TO_SECTOR.get(symbol.upper())
    if not sector:
        return [], None
    peers = [s for s in SECTOR_PEERS[sector] if s != symbol.upper()]
    return peers[:limit], sector


# Custom CSS for fixing dropdown visibility
app.index_string = '''
<!DOCTYPE html>
<html>
    <head>
        {%metas%}
        <title>{%title%}</title>
        {%favicon%}
        {%css%}
        <style>
            /* Fix dropdown menu visibility */
            .Select-menu-outer {
                background-color: #2c3034 !important;
                border: 1px solid #555 !important;
            }
            .Select-option {
                background-color: #2c3034 !important;
                color: #fff !important;
                padding: 8px 10px !important;
            }
            .Select-option:hover {
                background-color: #3e4449 !important;
                color: #fff !important;
            }
            .Select-option.is-focused {
                background-color: #3e4449 !important;
                color: #fff !important;
            }
            .Select-option.is-selected {
                background-color: #375a7f !important;
                color: #fff !important;
            }
            .Select-value-label {
                color: #fff !important;
            }
            .Select-placeholder {
                color: #aaa !important;
            }
            .Select-control {
                background-color: #2c3034 !important;
                border-color: #555 !important;
            }
        </style>
    </head>
    <body>
        {%app_entry%}
        <footer>
            {%config%}
            {%scripts%}
            {%renderer%}
        </footer>
    </body>
</html>
'''

# App layout
app.layout = dbc.Container([
    # Navigation Bar
    dbc.Navbar(
        dbc.Container([
            dbc.NavbarBrand("InvestIQ", className="ms-2 fw-bold", style={"color": "#00cc88", "fontSize": "1.3rem"}),
            dbc.Nav([
                dbc.NavItem(dbc.NavLink("Dashboard", id="nav-dashboard", href="#", active=True,
                                         style={"color": "#fff", "fontWeight": "500"})),
                dbc.NavItem(dbc.NavLink("Symbol Search", id="nav-search", href="#",
                                         style={"color": "#aaa"})),
            ], className="ms-auto", navbar=True),
        ], fluid=True),
        color="#1a1a2e",
        dark=True,
        className="mb-4",
        style={"borderBottom": "2px solid #00cc88"},
    ),

    # Page container — switches between dashboard and search
    # Dashboard page
    html.Div(id="page-dashboard", children=[

    # Header
    dbc.Row([
        dbc.Col([
            html.H1("Comprehensive Stock Analysis", className="text-center mb-4 mt-2"),
            html.P("AI-Powered Technical, Fundamental, Quantitative & Sentiment Analysis",
                   className="text-center text-muted mb-4")
        ])
    ]),

    # Stock Suggestions Section
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader([
                    dbc.Row([
                        dbc.Col([
                            html.H5("💡 Stock Suggestions", className="mb-0")
                        ], md=6),
                        dbc.Col([
                            dcc.Dropdown(
                                id='universe-dropdown',
                                options=[
                                    {'label': 'Popular Stocks', 'value': 'popular'},
                                    {'label': 'Tech Stocks', 'value': 'tech'},
                                    {'label': 'Blue Chips', 'value': 'bluechip'},
                                ],
                                value='popular',
                                className="mb-0"
                            ),
                        ], md=4),
                        dbc.Col([
                            dbc.Button(
                                "🔍 Get Suggestions",
                                id='suggest-button',
                                color="success",
                                className="w-100",
                                n_clicks=0
                            ),
                        ], md=2),
                    ])
                ]),
                dbc.CardBody([
                    dcc.Loading(
                        id="suggestions-loading",
                        children=html.Div(id='suggestions-display')
                    )
                ])
            ], className="mb-4")
        ])
    ]),

    # Search and Controls
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardBody([
                    dbc.Row([
                        dbc.Col([
                            dbc.Label("Stock Symbol"),
                            dbc.Input(
                                id='symbol-input',
                                type='text',
                                value='AAPL',
                                placeholder='Enter symbol (e.g., AAPL, MSFT, TSLA)',
                                className="mb-2"
                            ),
                        ], md=4),
                        dbc.Col([
                            dbc.Label("Timeframe"),
                            dcc.Dropdown(
                                id='timeframe-dropdown',
                                options=[
                                    {'label': '1 Minute', 'value': '1m'},
                                    {'label': '5 Minutes', 'value': '5m'},
                                    {'label': '15 Minutes', 'value': '15m'},
                                    {'label': '1 Hour', 'value': '1h'},
                                    {'label': '1 Day', 'value': '1d'},
                                    {'label': '1 Week', 'value': '1w'},
                                ],
                                value='1d',
                                className="mb-2"
                            ),
                        ], md=3),
                        dbc.Col([
                            dbc.Label("Days Back"),
                            dbc.Input(
                                id='days-input',
                                type='number',
                                value=90,
                                min=1,
                                max=1825,
                                className="mb-2"
                            ),
                        ], md=2),
                        dbc.Col([
                            dbc.Label(""),
                            html.Br(),
                            dbc.Button(
                                "🔍 Analyze",
                                id='analyze-button',
                                color="primary",
                                className="w-100",
                                n_clicks=0
                            ),
                        ], md=2),
                        dbc.Col([
                            dbc.Label(""),
                            html.Br(),
                            dbc.Button(
                                "🔄 Refresh",
                                id='refresh-button',
                                color="secondary",
                                className="w-100",
                                n_clicks=0
                            ),
                        ], md=1),
                    ])
                ])
            ], className="mb-4 dropdown-card")
        ])
    ]),

    # Loading indicator
    dbc.Row([
        dbc.Col([
            dcc.Loading(
                id="loading",
                type="default",
                children=html.Div(id="loading-output")
            )
        ])
    ]),

    # Overall Signal Card
    dbc.Row([
        dbc.Col([
            html.Div(id='overall-signal-card')
        ])
    ], className="mb-4"),

    # Paper Trading + Portfolio + Backtesting (tabbed)
    dcc.Store(id='paper-trade-symbol-store', data=''),
    dcc.Store(id='paper-trade-notification-store'),
    dcc.Store(id='live-trade-symbol-store', data=''),
    dcc.Store(id='live-trade-notification-store'),
    dbc.Card([
        dbc.CardHeader(
            dbc.Tabs([
                dbc.Tab(label="Paper Trade", tab_id="tab-trade"),
                dbc.Tab(label="Portfolio", tab_id="tab-portfolio"),
                dbc.Tab(label="Backtest", tab_id="tab-backtest"),
                dbc.Tab(label="Live Trade", tab_id="tab-live-trade",
                        label_style={"color": "#dc3545", "fontWeight": "bold"}),
                dbc.Tab(label="Agent Trades", tab_id="tab-agent-trades",
                        label_style={"color": "#ffc107"}),
            ], id="trading-tabs", active_tab="tab-trade", className="card-header-tabs")
        ),
        dbc.CardBody([
            html.Div(id='paper-trade-notification-area'),
            html.Div(id='live-trade-notification-area'),
            html.Div([
                dcc.Loading(html.Div(id='paper-trading-section'), type="circle"),
            ], id='tab-trade-content'),
            html.Div([
                html.Div(id='portfolio-order-notification-area'),
                dcc.Loading(html.Div(id='portfolio-dashboard-section'), type="circle"),
            ], id='tab-portfolio-content', style={'display': 'none'}),
            html.Div([
                dbc.Row([
                    dbc.Col([
                        dbc.InputGroup([
                            dbc.InputGroupText("Days"),
                            dbc.Input(
                                id="backtest-days-input",
                                type="number",
                                value=365,
                                min=90,
                                max=730,
                            ),
                        ], size="sm"),
                    ], md=3),
                    dbc.Col([
                        dbc.Button(
                            "Run Backtest",
                            id="backtest-run-btn",
                            color="primary",
                            size="sm",
                            className="w-100",
                        ),
                    ], md=2),
                    dbc.Col([
                        html.P(
                            "Simulates buy/sell signals over historical data. Results are hypothetical.",
                            className="text-muted small mb-0 pt-1",
                        ),
                    ], md=7),
                ], className="mb-3"),
                dcc.Loading(html.Div(id='backtest-results-section'), type="circle"),
                html.Hr(className="my-3"),
                dbc.Row([
                    dbc.Col([
                        dbc.Button(
                            "Run Monte Carlo (1000 sims)",
                            id="monte-carlo-btn",
                            color="info",
                            size="sm",
                            outline=True,
                            className="w-100",
                        ),
                    ], md=3),
                    dbc.Col([
                        dbc.Button(
                            "Run Walk-Forward Validation",
                            id="walk-forward-btn",
                            color="warning",
                            size="sm",
                            outline=True,
                            className="w-100",
                        ),
                    ], md=3),
                ], className="mb-3"),
                dcc.Loading(html.Div(id='monte-carlo-section'), type="circle"),
                dcc.Loading(html.Div(id='walk-forward-section'), type="circle"),
                dcc.Store(id='last-backtest-id', data=None),
            ], id='tab-backtest-content', style={'display': 'none'}),
            html.Div([
                dcc.Loading(html.Div(id='live-trading-section'), type="circle"),
            ], id='tab-live-trade-content', style={'display': 'none'}),
            html.Div([
                html.Div(id='agent-trade-notification-area'),
                dcc.Loading(html.Div(id='agent-trades-section'), type="circle"),
            ], id='tab-agent-trades-content', style={'display': 'none'}),
        ]),
    ], className="mb-4"),

    # Main Charts Row
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H5("📊 Price Chart with Technical Indicators")),
                dbc.CardBody([
                    dcc.Graph(id='main-chart', config={'displayModeBar': True})
                ])
            ])
        ], md=12)
    ], className="mb-4"),

    # Multi-Timeframe Mini-Charts Row
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H5("Daily (30d)", className="mb-0 small")),
                dbc.CardBody([
                    dcc.Loading(dcc.Graph(id='mini-chart-daily', config={'displayModeBar': False},
                        figure={'data': [], 'layout': {'height': 220,
                            'paper_bgcolor': 'rgba(0,0,0,0)', 'plot_bgcolor': 'rgba(0,0,0,0)',
                            'xaxis': {'visible': False}, 'yaxis': {'visible': False},
                            'annotations': [{'text': 'Click Analyze', 'xref': 'paper', 'yref': 'paper', 'x': 0.5, 'y': 0.5, 'showarrow': False, 'font': {'size': 14, 'color': '#888'}}]}}
                    ), type="circle")
                ], className="p-2")
            ])
        ], md=4),
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H5("Weekly (6mo)", className="mb-0 small")),
                dbc.CardBody([
                    dcc.Loading(dcc.Graph(id='mini-chart-weekly', config={'displayModeBar': False},
                        figure={'data': [], 'layout': {'height': 220,
                            'paper_bgcolor': 'rgba(0,0,0,0)', 'plot_bgcolor': 'rgba(0,0,0,0)',
                            'xaxis': {'visible': False}, 'yaxis': {'visible': False},
                            'annotations': [{'text': 'Click Analyze', 'xref': 'paper', 'yref': 'paper', 'x': 0.5, 'y': 0.5, 'showarrow': False, 'font': {'size': 14, 'color': '#888'}}]}}
                    ), type="circle")
                ], className="p-2")
            ])
        ], md=4),
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H5("Monthly (1yr)", className="mb-0 small")),
                dbc.CardBody([
                    dcc.Loading(dcc.Graph(id='mini-chart-monthly', config={'displayModeBar': False},
                        figure={'data': [], 'layout': {'height': 220,
                            'paper_bgcolor': 'rgba(0,0,0,0)', 'plot_bgcolor': 'rgba(0,0,0,0)',
                            'xaxis': {'visible': False}, 'yaxis': {'visible': False},
                            'annotations': [{'text': 'Click Analyze', 'xref': 'paper', 'yref': 'paper', 'x': 0.5, 'y': 0.5, 'showarrow': False, 'font': {'size': 14, 'color': '#888'}}]}}
                    ), type="circle")
                ], className="p-2")
            ])
        ], md=4),
    ], className="mb-4"),

    # Technical Indicators Row
    dbc.Row([
        dbc.Col([
            dbc.Card([
                dbc.CardHeader(html.H5("📈 Technical Indicators")),
                dbc.CardBody([
                    dcc.Graph(id='rsi-chart', config={'displayModeBar': False}),
                    dcc.Graph(id='macd-chart', config={'displayModeBar': False}),
                ])
            ])
        ], md=12)
    ], className="mb-4"),

    # Risk Radar & Confidence Gauge Row
    dbc.Row([
        dbc.Col([
            html.Div(id='risk-radar-section')
        ], md=6),
        dbc.Col([
            html.Div(id='confidence-section')
        ], md=6),
    ], className="mb-4"),

    # Sentiment Velocity Row
    dbc.Row([
        dbc.Col([
            html.Div(id='sentiment-velocity-section')
        ], md=12),
    ], className="mb-4"),

    # Peer Comparison Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(
                id="peer-loading",
                type="default",
                children=html.Div(id='peer-comparison-section')
            )
        ], md=12),
    ], className="mb-4"),

    # Analysis Results Row
    dbc.Row([
        # Technical Analysis
        dbc.Col([
            html.Div(id='technical-analysis-card')
        ], md=6),

        # Fundamental Analysis
        dbc.Col([
            html.Div(id='fundamental-analysis-card')
        ], md=6),
    ], className="mb-4"),

    # Quantitative & Sentiment Row
    dbc.Row([
        # Quantitative Analysis
        dbc.Col([
            html.Div(id='quant-analysis-card')
        ], md=6),

        # Sentiment Analysis
        dbc.Col([
            html.Div(id='sentiment-analysis-card')
        ], md=6),
    ], className="mb-4"),

    # Earnings & Dividend Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='earnings-section'), type="circle")
        ], md=6),
        dbc.Col([
            dcc.Loading(html.Div(id='dividend-section'), type="circle")
        ], md=6),
    ], className="mb-4"),

    # Options & Short Interest Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='options-section'), type="circle")
        ], md=8),
        dbc.Col([
            dcc.Loading(html.Div(id='short-interest-section'), type="circle")
        ], md=4),
    ], className="mb-4"),

    # Insider Activity Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='insider-section'), type="circle")
        ], md=12),
    ], className="mb-4"),

    # Correlation Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='correlation-section'), type="circle")
        ], md=12),
    ], className="mb-4"),

    # Social Sentiment & Macro Overlay Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='social-sentiment-section'), type="circle")
        ], md=6),
        dbc.Col([
            dcc.Loading(html.Div(id='macro-overlay-section'), type="circle")
        ], md=6),
    ], className="mb-4"),

    # Alpha Decay & Flow Map Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='alpha-decay-section'), type="circle")
        ], md=6),
        dbc.Col([
            dcc.Loading(html.Div(id='flow-map-section'), type="circle")
        ], md=6),
    ], className="mb-4"),

    # Smart Watchlist & Tax Dashboard Row
    dbc.Row([
        dbc.Col([
            dcc.Loading(html.Div(id='watchlist-section'), type="circle"),
            # Pagination controls (hidden until data loads)
            html.Div(id='watchlist-pagination', style={'display': 'none'}),
            html.Div(id='watchlist-cards'),
            dcc.Store(id='watchlist-store', data=None),
            dcc.Store(id='watchlist-page', data=0),
        ], md=6),
        dbc.Col([
            dcc.Loading(html.Div(id='tax-section'), type="circle")
        ], md=6),
    ], className="mb-4"),

    # Footer
    dbc.Row([
        dbc.Col([
            html.Hr(),
            html.P("Disclaimer: This tool is for educational purposes only. Not financial advice. Always consult a professional.",
                   className="text-center text-muted small")
        ])
    ])

    ]),  # End of page-dashboard div

    # Symbol Search page (hidden by default)
    html.Div(id="page-search", style={"display": "none"}, children=[
        SymbolSearchComponent.create_search_page_layout(),
    ]),

], fluid=True)


def _empty_results():
    """Return empty results tuple for all callback outputs"""
    empty_fig = create_empty_figure("Enter a symbol")
    return ("", create_empty_signal_card(), {}, {}, {},
            "", "", "",
            create_empty_card("Technical"), create_empty_card("Fundamental"),
            create_empty_card("Quantitative"), create_empty_card("Sentiment"),
            empty_fig, empty_fig, empty_fig)


def _error_results(error_msg):
    """Return error results tuple for all callback outputs"""
    empty_fig = create_empty_figure("No data")
    return (
        "",
        create_error_card(error_msg),
        {}, {}, {},
        "", "", "",
        create_empty_card("Technical"),
        create_empty_card("Fundamental"),
        create_empty_card("Quantitative"),
        create_empty_card("Sentiment"),
        empty_fig, empty_fig, empty_fig,
    )


def build_risk_radar_section(analysis, symbol):
    """Build risk radar UI from analysis data"""
    try:
        risk_scores = RiskRadarComponent.calculate_risk_from_analysis(analysis)
        radar_chart = create_risk_radar_chart(risk_scores, title=f"Risk Radar - {symbol}")
        risk_card = RiskRadarComponent.create_risk_card(risk_scores, symbol=symbol)
        breakdown = create_risk_breakdown_bars(risk_scores)
        return dbc.Card([
            dbc.CardHeader(html.H5("Risk Radar", className="mb-0")),
            dbc.CardBody([
                dcc.Graph(figure=radar_chart, config={'displayModeBar': False}),
                risk_card,
                dcc.Graph(figure=breakdown, config={'displayModeBar': False}),
            ])
        ], className="h-100")
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Risk Radar", className="mb-0")),
            dbc.CardBody(html.P(f"Could not load risk radar: {e}", className="text-muted"))
        ])


def build_confidence_section(symbol):
    """Build confidence gauge UI from calibrated analysis"""
    try:
        cal_data = ConfidenceGaugeComponent.fetch_calibrated_analysis(symbol)
        if not cal_data:
            return dbc.Card([
                dbc.CardHeader(html.H5("Confidence Compass", className="mb-0")),
                dbc.CardBody(html.P("No calibration data available. Analyze more symbols to build calibration history.", className="text-muted"))
            ], className="h-100")

        confidence_card = ConfidenceGaugeComponent.create_confidence_card(cal_data, symbol)

        calibrated = cal_data.get('calibrated', {})
        cal_conf = calibrated.get('calibrated_confidence', 0.5)
        lower = calibrated.get('lower_bound', cal_conf - 0.1)
        upper = calibrated.get('upper_bound', cal_conf + 0.1)
        original = cal_data.get('original_confidence', cal_conf)

        gauge_fig = create_confidence_gauge(cal_conf, lower, upper, original_confidence=original)

        return dbc.Card([
            dbc.CardHeader(html.H5("Confidence Compass", className="mb-0")),
            dbc.CardBody([
                dcc.Graph(figure=gauge_fig, config={'displayModeBar': False}),
                confidence_card,
            ])
        ], className="h-100")
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Confidence Compass", className="mb-0")),
            dbc.CardBody(html.P(f"Could not load confidence data: {e}", className="text-muted"))
        ], className="h-100")


def build_sentiment_velocity_section(symbol):
    """Build sentiment velocity UI"""
    try:
        velocity_data = SentimentVelocityComponent.fetch_velocity_data(symbol)
        history_data = SentimentVelocityComponent.fetch_history_data(symbol)

        velocity_card = SentimentVelocityComponent.create_velocity_card(velocity_data, symbol)

        children = []

        if velocity_data:
            dynamics = velocity_data.get('dynamics', {})
            gauge_fig = create_velocity_gauge(
                dynamics.get('velocity', 0),
                dynamics.get('current_sentiment', 0),
                dynamics.get('signal', 'Stable'),
            )
            children.append(dbc.Row([
                dbc.Col([
                    dcc.Graph(figure=gauge_fig, config={'displayModeBar': False}),
                    velocity_card,
                ], md=5),
                dbc.Col([
                    dcc.Graph(
                        figure=create_sentiment_history_chart(
                            history_data.get('history', []) if history_data else [],
                            symbol,
                        ),
                        config={'displayModeBar': False},
                    ),
                ], md=7),
            ]))
        else:
            children.append(velocity_card)

        return dbc.Card([
            dbc.CardHeader(html.H5("Sentiment Velocity", className="mb-0")),
            dbc.CardBody(children)
        ])
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Sentiment Velocity", className="mb-0")),
            dbc.CardBody(html.P(f"Could not load sentiment velocity: {e}", className="text-muted"))
        ])


# Sensible defaults per timeframe: (default_days, min_days, max_days)
TIMEFRAME_DAYS = {
    '1m':  (1,   1,   7),
    '5m':  (3,   1,   30),
    '15m': (7,   1,   60),
    '1h':  (30,  1,   180),
    '1d':  (90,  1,   1825),
    '1w':  (365, 7,   1825),
}


@app.callback(
    [Output('days-input', 'value'),
     Output('days-input', 'min'),
     Output('days-input', 'max')],
    Input('timeframe-dropdown', 'value'),
)
def sync_days_to_timeframe(timeframe):
    default, mn, mx = TIMEFRAME_DAYS.get(timeframe, (90, 1, 365))
    return default, mn, mx


# Callback for analysis (background + cancel: re-clicking Analyze kills the previous run)
@app.callback(
    [
        Output('loading-output', 'children'),
        Output('overall-signal-card', 'children'),
        Output('main-chart', 'figure'),
        Output('rsi-chart', 'figure'),
        Output('macd-chart', 'figure'),
        Output('risk-radar-section', 'children'),
        Output('confidence-section', 'children'),
        Output('sentiment-velocity-section', 'children'),
        Output('technical-analysis-card', 'children'),
        Output('fundamental-analysis-card', 'children'),
        Output('quant-analysis-card', 'children'),
        Output('sentiment-analysis-card', 'children'),
        Output('mini-chart-daily', 'figure'),
        Output('mini-chart-weekly', 'figure'),
        Output('mini-chart-monthly', 'figure'),
    ],
    [
        Input('analyze-button', 'n_clicks'),
        Input('refresh-button', 'n_clicks'),
    ],
    [
        State('symbol-input', 'value'),
        State('timeframe-dropdown', 'value'),
        State('days-input', 'value'),
    ],
)
def update_analysis(analyze_clicks, refresh_clicks, symbol, timeframe, days):
    if not symbol:
        return _empty_results()

    symbol = symbol.upper()

    try:
        # Fetch analysis and bars in parallel
        with ThreadPoolExecutor(max_workers=2) as executor:
            f_analysis = executor.submit(
                requests.get, f'{API_BASE_URL}/api/analyze/{symbol}',
                params={'timeframe': timeframe, 'days': days},
                headers=get_headers(), timeout=30
            )
            f_bars = executor.submit(
                requests.get, f'{API_BASE_URL}/api/bars/{symbol}',
                params={'timeframe': timeframe, 'days': days},
                headers=get_headers(), timeout=30
            )
            analysis_response = f_analysis.result()
            bars_response = f_bars.result()

        # Check if response is JSON
        try:
            analysis_data = analysis_response.json()
        except requests.exceptions.JSONDecodeError:
            error_msg = f"API Error: {analysis_response.text[:200]}"
            return _error_results(error_msg)

        try:
            bars_data = bars_response.json()
        except requests.exceptions.JSONDecodeError:
            bars_data = {'success': False, 'data': []}

        if not analysis_data.get('success'):
            error_msg = analysis_data.get('error', 'Unknown error')
            return _error_results(error_msg)

        analysis = analysis_data['data']

        # Cache the analysis result for reuse by other callbacks
        _cache_set(f'analysis:{symbol}', analysis)

        # Parse bars
        bars = None
        if bars_data.get('success') and bars_data.get('data'):
            bars = pd.DataFrame(bars_data['data'])
            bars.loc[:, 'timestamp'] = pd.to_datetime(bars['timestamp'])

        # Fallback: if current_price is missing, get from bars
        if not analysis.get('current_price') and bars is not None and not bars.empty:
            analysis['current_price'] = float(bars['close'].iloc[-1])

        # Create overall signal card
        signal_card = create_signal_card(analysis)

        # Create charts
        if bars is not None and not bars.empty:
            main_chart = create_main_chart(bars, symbol, analysis)
            rsi_chart = create_rsi_chart(bars, analysis)
            macd_chart = create_macd_chart(bars, analysis)
        else:
            main_chart = create_empty_figure("No price data available")
            rsi_chart = create_empty_figure("No data")
            macd_chart = create_empty_figure("No data")

        # Build risk radar (uses analysis data already fetched, no API call)
        risk_radar = build_risk_radar_section(analysis, symbol)

        # Create analysis cards (no API calls)
        tech_card = create_technical_card(analysis.get('technical'))
        fund_card = create_fundamental_card(analysis.get('fundamental'))
        quant_card = create_quant_card(analysis.get('quantitative'))
        sent_card = create_sentiment_card(analysis.get('sentiment'))

        # Fetch confidence, sentiment velocity, and daily bars in parallel
        # Daily bars (365d) are resampled to weekly/monthly locally to avoid
        # exhausting the Polygon rate limit (5 req/min).
        with ThreadPoolExecutor(max_workers=3) as executor:
            f_confidence = executor.submit(build_confidence_section, symbol)
            f_velocity = executor.submit(build_sentiment_velocity_section, symbol)
            f_daily = executor.submit(_fetch_bars, symbol, '1d', 365)

            confidence = f_confidence.result()
            sentiment_velocity = f_velocity.result()
            daily_bars_full = f_daily.result()

        # Slice / resample the single daily dataset for each timeframe
        daily_bars = _slice_recent(daily_bars_full, 30)
        weekly_bars = _resample_bars(daily_bars_full, 'W')
        monthly_bars = _resample_bars(daily_bars_full, 'ME')

        mini_daily = create_mini_chart(daily_bars, symbol, "Daily")
        mini_weekly = create_mini_chart(weekly_bars, symbol, "Weekly")
        mini_monthly = create_mini_chart(monthly_bars, symbol, "Monthly")

        return ("", signal_card, main_chart, rsi_chart, macd_chart,
                risk_radar, confidence, sentiment_velocity,
                tech_card, fund_card, quant_card, sent_card,
                mini_daily, mini_weekly, mini_monthly)

    except Exception as e:
        error_msg = f"Error: {str(e)}"
        return _error_results(error_msg)


def create_signal_card(analysis):
    """Create overall signal summary card"""
    signal = analysis['overall_signal']
    confidence = analysis['overall_confidence'] * 100

    # Signal emoji and color
    signal_map = {
        'StrongBuy': ('🚀', 'success', '#00ff00'),
        'Buy': ('📈', 'success', '#00cc00'),
        'WeakBuy': ('↗️', 'info', '#00aaaa'),
        'Neutral': ('➡️', 'warning', '#ffaa00'),
        'WeakSell': ('↘️', 'warning', '#ff8800'),
        'Sell': ('📉', 'danger', '#ff4400'),
        'StrongSell': ('⚠️', 'danger', '#ff0000'),
    }

    emoji, color, _ = signal_map.get(signal, ('➡️', 'secondary', '#888888'))

    # Current price
    current_price = analysis.get('current_price')
    price_display = html.H2(f"${current_price:,.2f}", className="mb-0") if current_price else html.H4("Price N/A", className="mb-0 text-muted")

    # Signal agreement across engines
    bullish_signals = {'StrongBuy', 'Buy', 'WeakBuy'}
    bearish_signals = {'StrongSell', 'Sell', 'WeakSell'}
    engines = ['technical', 'fundamental', 'quantitative', 'sentiment']
    bullish_count = 0
    bearish_count = 0
    total_engines = 0
    for engine in engines:
        engine_data = analysis.get(engine)
        if engine_data:
            total_engines += 1
            eng_signal = engine_data.get('signal', 'Neutral')
            if eng_signal in bullish_signals:
                bullish_count += 1
            elif eng_signal in bearish_signals:
                bearish_count += 1

    if total_engines > 0:
        if bullish_count > bearish_count:
            agreement_text = f"{bullish_count} of {total_engines} engines bullish"
            agreement_color = "text-success"
        elif bearish_count > bullish_count:
            agreement_text = f"{bearish_count} of {total_engines} engines bearish"
            agreement_color = "text-danger"
        else:
            agreement_text = f"Engines split ({bullish_count} bull / {bearish_count} bear)"
            agreement_color = "text-warning"
    else:
        agreement_text = ""
        agreement_color = "text-muted"

    return dbc.Card([
        dbc.CardBody([
            dbc.Row([
                dbc.Col([
                    price_display,
                    html.P(analysis.get('name') or analysis['symbol'], className="mb-0 text-muted"),
                ], md=2, className="text-center d-flex flex-column justify-content-center"),
                dbc.Col([
                    html.H2(f"{emoji} {signal}", className="text-center mb-1"),
                    html.P(analysis['recommendation'], className="text-center text-muted mb-0 small"),
                    html.P(agreement_text, className=f"text-center {agreement_color} mb-0 small") if agreement_text else None,
                ], md=4),
                dbc.Col([
                    html.Div([
                        html.H6("Confidence Score"),
                        dbc.Progress(value=confidence, label=f"{confidence:.1f}%",
                                   color=color, className="mb-2", style={'height': '30px'}),
                    ])
                ], md=4),
                dbc.Col([
                    html.Div([
                        html.P(f"Analysis Time:", className="mb-0 small text-muted"),
                        html.P(f"{analysis['timestamp'][:19]}", className="mb-0 small"),
                    ])
                ], md=2),
            ])
        ])
    ], color=color, outline=True, className="mb-4")


def create_main_chart(df, symbol, analysis):
    """Create main candlestick chart with volume, SMAs, VWAP, and Fibonacci levels"""
    fig = make_subplots(
        rows=2, cols=1,
        shared_xaxes=True,
        vertical_spacing=0.03,
        row_heights=[0.7, 0.3],
        subplot_titles=(f'{analysis.get("name") or symbol} ({symbol}) Price', 'Volume')
    )

    # Candlestick
    fig.add_trace(
        go.Candlestick(
            x=df['timestamp'],
            open=df['open'],
            high=df['high'],
            low=df['low'],
            close=df['close'],
            name='Price'
        ),
        row=1, col=1
    )

    # Add Bollinger Bands + SMA 20 if we have enough data
    if len(df) >= 20:
        sma_20 = df['close'].rolling(window=20).mean()
        std_20 = df['close'].rolling(window=20).std()
        upper_band = sma_20 + (std_20 * 2)
        lower_band = sma_20 - (std_20 * 2)

        fig.add_trace(
            go.Scatter(x=df['timestamp'], y=upper_band, name='BB Upper',
                      line=dict(color='rgba(250, 250, 250, 0.3)', width=1)),
            row=1, col=1
        )
        fig.add_trace(
            go.Scatter(x=df['timestamp'], y=sma_20, name='SMA 20',
                      line=dict(color='orange', width=1)),
            row=1, col=1
        )
        fig.add_trace(
            go.Scatter(x=df['timestamp'], y=lower_band, name='BB Lower',
                      line=dict(color='rgba(250, 250, 250, 0.3)', width=1),
                      fill='tonexty'),
            row=1, col=1
        )

    # SMA 50
    if len(df) >= 50:
        sma_50 = df['close'].rolling(window=50).mean()
        fig.add_trace(
            go.Scatter(x=df['timestamp'], y=sma_50, name='SMA 50',
                      line=dict(color='cyan', width=1, dash='dot')),
            row=1, col=1
        )

    # VWAP (Volume Weighted Average Price)
    if 'volume' in df.columns and df['volume'].sum() > 0:
        typical_price = (df['high'] + df['low'] + df['close']) / 3
        cum_tp_vol = (typical_price * df['volume']).cumsum()
        cum_vol = df['volume'].cumsum()
        vwap = cum_tp_vol / cum_vol
        fig.add_trace(
            go.Scatter(x=df['timestamp'], y=vwap, name='VWAP',
                      line=dict(color='#bb86fc', width=1.5, dash='dash')),
            row=1, col=1
        )

    # Fibonacci retracement levels
    if len(df) >= 10:
        period_high = df['high'].max()
        period_low = df['low'].min()
        diff = period_high - period_low
        if diff > 0:
            fib_levels = [
                (0.0, period_high, "0%"),
                (0.236, period_high - diff * 0.236, "23.6%"),
                (0.382, period_high - diff * 0.382, "38.2%"),
                (0.5, period_high - diff * 0.5, "50%"),
                (0.618, period_high - diff * 0.618, "61.8%"),
                (0.786, period_high - diff * 0.786, "78.6%"),
                (1.0, period_low, "100%"),
            ]
            for _, level, label in fib_levels:
                fig.add_hline(
                    y=level, line_dash="dot",
                    line_color="rgba(255, 215, 0, 0.3)", line_width=1,
                    annotation_text=f"Fib {label}",
                    annotation_position="right",
                    annotation_font=dict(size=9, color="rgba(255, 215, 0, 0.6)"),
                    row=1, col=1,
                )

    # Candlestick pattern annotations
    detected_patterns = (analysis.get('technical') or {}).get('metrics', {}).get('detected_patterns', []) if analysis else []
    if detected_patterns and len(df) > 0:
        bullish_x, bullish_y, bullish_text = [], [], []
        bearish_x, bearish_y, bearish_text = [], [], []
        neutral_x, neutral_y, neutral_text = [], [], []

        for p in detected_patterns:
            idx = p.get('index', 0)
            if idx < len(df):
                row = df.iloc[idx]
                name = p.get('name', 'Pattern')
                strength = p.get('strength', 0)
                is_bullish = p.get('bullish', False)
                is_neutral = name == 'Doji'
                price_range = df['high'].max() - df['low'].min()
                offset = price_range * 0.02

                hover = f"{name} (strength: {strength:.0%})"
                if is_neutral:
                    neutral_x.append(row['timestamp'])
                    neutral_y.append(row['high'] + offset)
                    neutral_text.append(hover)
                elif is_bullish:
                    bullish_x.append(row['timestamp'])
                    bullish_y.append(row['low'] - offset)
                    bullish_text.append(hover)
                else:
                    bearish_x.append(row['timestamp'])
                    bearish_y.append(row['high'] + offset)
                    bearish_text.append(hover)

        if bullish_x:
            fig.add_trace(go.Scatter(
                x=bullish_x, y=bullish_y, mode='markers+text',
                marker=dict(symbol='triangle-up', size=12, color='#00ff00'),
                text=[p.split(' (')[0] for p in bullish_text],
                textposition='bottom center', textfont=dict(size=9, color='#00ff00'),
                hovertext=bullish_text, hoverinfo='text',
                name='Bullish Pattern', showlegend=True,
            ), row=1, col=1)
        if bearish_x:
            fig.add_trace(go.Scatter(
                x=bearish_x, y=bearish_y, mode='markers+text',
                marker=dict(symbol='triangle-down', size=12, color='#ff4444'),
                text=[p.split(' (')[0] for p in bearish_text],
                textposition='top center', textfont=dict(size=9, color='#ff4444'),
                hovertext=bearish_text, hoverinfo='text',
                name='Bearish Pattern', showlegend=True,
            ), row=1, col=1)
        if neutral_x:
            fig.add_trace(go.Scatter(
                x=neutral_x, y=neutral_y, mode='markers+text',
                marker=dict(symbol='diamond', size=10, color='#ffaa00'),
                text=[p.split(' (')[0] for p in neutral_text],
                textposition='top center', textfont=dict(size=9, color='#ffaa00'),
                hovertext=neutral_text, hoverinfo='text',
                name='Neutral Pattern', showlegend=True,
            ), row=1, col=1)

    # Volume bars
    colors = ['red' if df['close'].iloc[i] < df['open'].iloc[i] else 'green'
              for i in range(len(df))]

    fig.add_trace(
        go.Bar(x=df['timestamp'], y=df['volume'], name='Volume',
               marker_color=colors, showlegend=False),
        row=2, col=1
    )

    fig.update_layout(
        height=650,
        template='plotly_dark',
        xaxis_rangeslider_visible=False,
        hovermode='x unified',
        legend=dict(orientation="h", yanchor="bottom", y=1.02, xanchor="right", x=1),
    )

    fig.update_xaxes(title_text="Date", row=2, col=1)
    fig.update_yaxes(title_text="Price ($)", row=1, col=1)
    fig.update_yaxes(title_text="Volume", row=2, col=1)

    return fig


def create_rsi_chart(df, analysis):
    """Create RSI indicator chart"""
    if len(df) < 14:
        return create_empty_figure("Insufficient data for RSI")

    # Calculate RSI
    delta = df['close'].diff()
    gain = (delta.where(delta > 0, 0)).rolling(window=14).mean()
    loss = (-delta.where(delta < 0, 0)).rolling(window=14).mean()
    rs = gain / loss
    rsi = 100 - (100 / (1 + rs))

    fig = go.Figure()

    fig.add_trace(go.Scatter(
        x=df['timestamp'],
        y=rsi,
        mode='lines',
        name='RSI',
        line=dict(color='cyan', width=2)
    ))

    # Add reference lines
    fig.add_hline(y=70, line_dash="dash", line_color="red", annotation_text="Overbought")
    fig.add_hline(y=30, line_dash="dash", line_color="green", annotation_text="Oversold")
    fig.add_hline(y=50, line_dash="dot", line_color="gray")

    # Highlight current RSI value
    if analysis.get('technical') and analysis['technical'].get('metrics', {}).get('rsi'):
        current_rsi = analysis['technical']['metrics']['rsi']
        fig.add_annotation(
            x=df['timestamp'].iloc[-1],
            y=current_rsi,
            text=f"RSI: {current_rsi:.1f}",
            showarrow=True,
            arrowhead=2,
            bgcolor="cyan",
            font=dict(color="black")
        )

    fig.update_layout(
        height=250,
        template='plotly_dark',
        yaxis_title="RSI",
        yaxis=dict(range=[0, 100]),
        hovermode='x unified',
        showlegend=False
    )

    return fig


def create_macd_chart(df, analysis):
    """Create MACD indicator chart"""
    if len(df) < 26:
        return create_empty_figure("Insufficient data for MACD")

    # Calculate MACD
    exp1 = df['close'].ewm(span=12, adjust=False).mean()
    exp2 = df['close'].ewm(span=26, adjust=False).mean()
    macd = exp1 - exp2
    signal = macd.ewm(span=9, adjust=False).mean()
    histogram = macd - signal

    fig = go.Figure()

    fig.add_trace(go.Scatter(
        x=df['timestamp'],
        y=macd,
        mode='lines',
        name='MACD',
        line=dict(color='blue', width=2)
    ))

    fig.add_trace(go.Scatter(
        x=df['timestamp'],
        y=signal,
        mode='lines',
        name='Signal',
        line=dict(color='orange', width=2)
    ))

    # Histogram
    colors = ['green' if val >= 0 else 'red' for val in histogram]
    fig.add_trace(go.Bar(
        x=df['timestamp'],
        y=histogram,
        name='Histogram',
        marker_color=colors
    ))

    fig.add_hline(y=0, line_dash="dash", line_color="gray")

    fig.update_layout(
        height=250,
        template='plotly_dark',
        yaxis_title="MACD",
        hovermode='x unified'
    )

    return fig


def _format_metric(value, fmt=".2f", suffix="", prefix=""):
    """Format a metric value safely"""
    if value is None:
        return "N/A"
    try:
        return f"{prefix}{value:{fmt}}{suffix}"
    except (ValueError, TypeError):
        return str(value)


def _trend_icon(trend):
    """Return trend direction text"""
    if not trend:
        return "N/A"
    t = str(trend).lower()
    if 'up' in t:
        return f"Uptrend"
    elif 'down' in t:
        return f"Downtrend"
    return f"{trend}"


def create_technical_card(tech_data):
    """Create technical analysis card"""
    if not tech_data:
        return create_empty_card("Technical Analysis")

    metrics = tech_data.get('metrics', {})
    rsi = metrics.get('rsi')
    macd_hist = metrics.get('macd_histogram')
    trend = metrics.get('trend', 'N/A')
    patterns = metrics.get('patterns', 0)
    signal_count = metrics.get('signal_count', 0)

    # RSI interpretation
    rsi_text = "N/A"
    rsi_class = ""
    if rsi is not None:
        rsi_text = f"{rsi:.1f}"
        if rsi > 70:
            rsi_class = "text-danger"
            rsi_text += " (Overbought)"
        elif rsi < 30:
            rsi_class = "text-success"
            rsi_text += " (Oversold)"

    return dbc.Card([
        dbc.CardHeader([
            html.H5("Technical Analysis", className="mb-0")
        ]),
        dbc.CardBody([
            html.H6(f"Signal: {tech_data['signal']}", className="mb-2"),
            dbc.Progress(
                value=tech_data['confidence'] * 100,
                label=f"{tech_data['confidence']*100:.0f}% Confidence",
                className="mb-3"
            ),
            html.Hr(),
            html.P(tech_data['reason'], className="small"),
            html.Hr(),
            html.H6("Key Metrics:", className="mt-3"),
            html.Ul([
                html.Li([html.Strong("RSI: "), html.Span(rsi_text, className=rsi_class)]),
                html.Li(f"MACD Histogram: {_format_metric(macd_hist, '.4f')}" if macd_hist else "MACD Histogram: N/A"),
                html.Li(f"Trend: {_trend_icon(trend)}"),
                html.Li([
                    html.Strong(f"Patterns ({patterns}): "),
                    ", ".join(
                        f"{'+ ' if p.get('bullish') else '- '}{p['name']}"
                        for p in metrics.get('detected_patterns', [])
                    ) if metrics.get('detected_patterns') else "None"
                ]),
                html.Li(f"Total Signals: {signal_count}"),
            ])
        ])
    ], className="h-100")


def _format_large_number(value):
    """Format a large number with B/M/K suffixes"""
    if value is None:
        return "N/A"
    try:
        v = float(value)
        if abs(v) >= 1e9:
            return f"${v/1e9:.2f}B"
        elif abs(v) >= 1e6:
            return f"${v/1e6:.2f}M"
        elif abs(v) >= 1e3:
            return f"${v/1e3:.1f}K"
        return f"${v:,.0f}"
    except (ValueError, TypeError):
        return str(value)


def create_fundamental_card(fund_data):
    """Create fundamental analysis card"""
    if not fund_data:
        return create_empty_card("Fundamental Analysis")

    metrics = fund_data.get('metrics', {})

    metric_items = [
        html.Li(f"P/E Ratio: {_format_metric(metrics.get('pe_ratio'))}"),
        html.Li(f"ROE: {_format_metric(metrics.get('roe'), suffix='%')}"),
        html.Li(f"Profit Margin: {_format_metric(metrics.get('profit_margin'), suffix='%')}"),
        html.Li(f"Debt/Equity: {_format_metric(metrics.get('debt_to_equity'))}"),
        html.Li(f"Current Ratio: {_format_metric(metrics.get('current_ratio'))}"),
    ]

    # Add revenue and cash flow if available
    revenue = metrics.get('revenue')
    if revenue:
        metric_items.append(html.Li(f"Revenue: {_format_large_number(revenue)}"))
    cash_flow = metrics.get('operating_cash_flow')
    if cash_flow:
        metric_items.append(html.Li(f"Operating Cash Flow: {_format_large_number(cash_flow)}"))

    # Build analyst consensus section if data present
    analyst_section = []
    analyst_target = metrics.get('analyst_price_target')
    if analyst_target is not None:
        upside_pct = metrics.get('analyst_upside_pct', 0)
        consensus_rating = metrics.get('analyst_consensus_rating', 'N/A')
        analyst_count = metrics.get('analyst_count')
        high_target = metrics.get('analyst_high_target')
        low_target = metrics.get('analyst_low_target')
        upgrades = metrics.get('analyst_upgrades_recent', 0)
        downgrades = metrics.get('analyst_downgrades_recent', 0)

        upside_color = "text-success" if upside_pct > 0 else "text-danger"
        upside_sign = "+" if upside_pct > 0 else ""

        analyst_items = [
            html.Li([
                html.Strong("Consensus: "),
                html.Span(str(consensus_rating)),
                html.Span(f" ({analyst_count} analysts)", className="text-muted small") if analyst_count else None,
            ]),
            html.Li([
                html.Strong("Price Target: "),
                html.Span(f"${analyst_target:,.2f} "),
                html.Span(f"({upside_sign}{upside_pct:.1f}%)", className=upside_color),
            ]),
        ]
        if high_target is not None and low_target is not None:
            analyst_items.append(
                html.Li(f"Target Range: ${low_target:,.2f} - ${high_target:,.2f}")
            )
        if upgrades or downgrades:
            net = upgrades - downgrades
            momentum_text = f"{upgrades} upgrades, {downgrades} downgrades"
            momentum_color = "text-success" if net > 0 else "text-danger" if net < 0 else ""
            analyst_items.append(
                html.Li([html.Strong("Recent Momentum: "), html.Span(momentum_text, className=momentum_color)])
            )

        analyst_section = [
            html.Hr(),
            html.H6("Analyst Consensus:", className="mt-3"),
            html.Ul(analyst_items),
        ]

    return dbc.Card([
        dbc.CardHeader([
            html.H5("Fundamental Analysis", className="mb-0")
        ]),
        dbc.CardBody([
            html.H6(f"Signal: {fund_data['signal']}", className="mb-2"),
            dbc.Progress(
                value=fund_data['confidence'] * 100,
                label=f"{fund_data['confidence']*100:.0f}% Confidence",
                className="mb-3"
            ),
            html.Hr(),
            html.P(fund_data['reason'], className="small"),
            html.Hr(),
            html.H6("Financial Metrics:", className="mt-3"),
            html.Ul(metric_items),
            *analyst_section,
        ])
    ], className="h-100")


def create_quant_card(quant_data):
    """Create quantitative analysis card"""
    if not quant_data:
        return create_empty_card("Quantitative Analysis")

    metrics = quant_data.get('metrics', {})

    # Win rate comes as 0-1 from the API, display as percentage
    win_rate = metrics.get('win_rate')
    win_rate_text = f"{win_rate*100:.1f}%" if win_rate is not None else "N/A"

    recent_return = metrics.get('recent_return')

    metric_items = [
        html.Li(f"Sharpe Ratio: {_format_metric(metrics.get('sharpe_ratio'), '.3f')}"),
        html.Li(f"Volatility: {_format_metric(metrics.get('volatility'), '.2f', suffix='%')}"),
        html.Li(f"Max Drawdown: {_format_metric(metrics.get('max_drawdown'), '.2f', suffix='%')}"),
        html.Li(f"Beta: {_format_metric(metrics.get('beta'), '.3f')}"),
        html.Li(f"VaR (95%): {_format_metric(metrics.get('var_95'), '.2f', suffix='%')}"),
        html.Li(f"Win Rate: {win_rate_text}"),
    ]

    if recent_return is not None:
        ret_class = "text-success" if recent_return > 0 else "text-danger" if recent_return < 0 else ""
        metric_items.append(html.Li([
            "20-Day Return: ",
            html.Span(f"{recent_return:+.2f}%", className=ret_class),
        ]))

    return dbc.Card([
        dbc.CardHeader([
            html.H5("Quantitative Analysis", className="mb-0")
        ]),
        dbc.CardBody([
            html.H6(f"Signal: {quant_data['signal']}", className="mb-2"),
            dbc.Progress(
                value=quant_data['confidence'] * 100,
                label=f"{quant_data['confidence']*100:.0f}% Confidence",
                className="mb-3"
            ),
            html.Hr(),
            html.P(quant_data['reason'], className="small"),
            html.Hr(),
            html.H6("Risk Metrics:", className="mt-3"),
            html.Ul(metric_items)
        ])
    ], className="h-100")


def create_sentiment_card(sent_data):
    """Create sentiment analysis card"""
    if not sent_data:
        return create_empty_card("Sentiment Analysis")

    metrics = sent_data.get('metrics', {})

    sentiment_score = metrics.get('normalized_score', 0)
    avg_sentiment = metrics.get('avg_sentiment')

    # Sentiment bar: map -100..100 to 0..100 for progress bar
    bar_value = (sentiment_score + 100) / 2  # -100->0, 0->50, 100->100
    if sentiment_score > 20:
        bar_color = "success"
    elif sentiment_score > -20:
        bar_color = "warning"
    else:
        bar_color = "danger"

    return dbc.Card([
        dbc.CardHeader([
            html.H5("Sentiment Analysis", className="mb-0")
        ]),
        dbc.CardBody([
            html.H6(f"Signal: {sent_data['signal']}", className="mb-2"),
            dbc.Progress(
                value=sent_data['confidence'] * 100,
                label=f"{sent_data['confidence']*100:.0f}% Confidence",
                className="mb-3"
            ),
            html.Hr(),
            html.P(sent_data['reason'], className="small"),
            html.Hr(),
            # Sentiment score visual bar
            html.H6("Sentiment Score:", className="mt-3 mb-1"),
            html.Div([
                html.Div([
                    html.Small("Bearish", className="text-danger"),
                    html.Small(f"{sentiment_score:+.1f}", className="fw-bold"),
                    html.Small("Bullish", className="text-success"),
                ], className="d-flex justify-content-between mb-1"),
                dbc.Progress(value=bar_value, color=bar_color, style={'height': '12px'}),
            ], className="mb-3"),
            html.Hr(),
            html.H6("News Breakdown:", className="mt-3"),
            dbc.Row([
                dbc.Col([
                    html.Div([
                        html.H4(metrics.get('positive_articles', 0), className="text-success"),
                        html.P("Positive", className="small")
                    ], className="text-center")
                ], width=4),
                dbc.Col([
                    html.Div([
                        html.H4(metrics.get('neutral_articles', 0), className="text-warning"),
                        html.P("Neutral", className="small")
                    ], className="text-center")
                ], width=4),
                dbc.Col([
                    html.Div([
                        html.H4(metrics.get('negative_articles', 0), className="text-danger"),
                        html.P("Negative", className="small")
                    ], className="text-center")
                ], width=4),
            ]),
            html.Hr(),
            html.P(f"Total Articles: {metrics.get('total_articles', 0)}", className="small text-muted mb-1"),
            html.P(f"Avg Sentiment: {_format_metric(avg_sentiment, '+.2f')}", className="small text-muted mb-0") if avg_sentiment is not None else None,
        ])
    ], className="h-100")


def create_empty_card(title):
    """Create empty card placeholder"""
    return dbc.Card([
        dbc.CardHeader([html.H5(f"{title}", className="mb-0")]),
        dbc.CardBody([
            html.P("No data available", className="text-muted text-center")
        ])
    ], className="h-100")


def create_empty_signal_card():
    """Create empty signal card"""
    return dbc.Card([
        dbc.CardBody([
            html.P("Enter a symbol and click Analyze to get started", className="text-center text-muted")
        ])
    ])


def create_error_card(error_msg):
    """Create error card"""
    return dbc.Card([
        dbc.CardBody([
            html.H5("❌ Error", className="text-danger"),
            html.P(error_msg, className="text-muted")
        ])
    ], color="danger", outline=True)


def create_empty_figure(message):
    """Create empty figure with message"""
    fig = go.Figure()
    fig.add_annotation(
        text=message,
        xref="paper",
        yref="paper",
        x=0.5,
        y=0.5,
        showarrow=False,
        font=dict(size=16, color="gray")
    )
    fig.update_layout(
        template='plotly_dark',
        height=220,
        xaxis=dict(showgrid=False, showticklabels=False),
        yaxis=dict(showgrid=False, showticklabels=False)
    )
    return fig


# --- Multi-Timeframe Mini-Charts ---

def create_mini_chart(df, symbol, timeframe_label):
    """Create a compact candlestick chart for multi-timeframe view"""
    if df is None or df.empty:
        return create_empty_figure(f"No {timeframe_label} data")

    fig = go.Figure()

    fig.add_trace(go.Candlestick(
        x=df['timestamp'],
        open=df['open'],
        high=df['high'],
        low=df['low'],
        close=df['close'],
        name='Price',
        increasing_line_color='#00cc88',
        decreasing_line_color='#ff4444',
    ))

    # Add SMA 20 if enough data
    if len(df) >= 20:
        sma = df['close'].rolling(window=20).mean()
        fig.add_trace(go.Scatter(
            x=df['timestamp'], y=sma, name='SMA 20',
            line=dict(color='orange', width=1),
            showlegend=False,
        ))

    fig.update_layout(
        height=220,
        template='plotly_dark',
        xaxis_rangeslider_visible=False,
        showlegend=False,
        margin=dict(l=40, r=10, t=10, b=20),
        xaxis=dict(showgrid=False),
        yaxis=dict(showgrid=True, gridcolor='rgba(128,128,128,0.15)'),
    )

    return fig


def _fetch_bars(symbol, timeframe, days):
    """Fetch bars for a given timeframe, return DataFrame or None"""
    try:
        timeout = 300
        resp = requests.get(
            f'{API_BASE_URL}/api/bars/{symbol}',
            params={'timeframe': timeframe, 'days': days},
            headers=get_headers(),
            timeout=timeout,
        )
        data = resp.json()
        if data.get('success') and data.get('data'):
            df = pd.DataFrame(data['data'])
            if df.empty:
                print(f"[mini-chart] No bars for {symbol} ({timeframe}, {days}d)")
                return None
            df.loc[:, 'timestamp'] = pd.to_datetime(df['timestamp'])
            return df
        else:
            print(f"[mini-chart] API returned no data for {symbol} ({timeframe}, {days}d): success={data.get('success')}, data_len={len(data.get('data') or [])}")
    except Exception as e:
        print(f"[mini-chart] Error fetching bars for {symbol} ({timeframe}, {days}d): {e}")
    return None


def _slice_recent(df, days):
    """Return the last N calendar days from a bars DataFrame."""
    if df is None or df.empty:
        return df
    cutoff = df['timestamp'].max() - pd.Timedelta(days=days)
    return df[df['timestamp'] >= cutoff].copy()


def _resample_bars(df, freq):
    """Resample daily bars to weekly ('W') or monthly ('ME') OHLCV candles."""
    if df is None or df.empty:
        return df
    tmp = df.set_index('timestamp').sort_index()
    resampled = tmp.resample(freq).agg({
        'open': 'first',
        'high': 'max',
        'low': 'min',
        'close': 'last',
        'volume': 'sum',
    }).dropna(subset=['open'])
    resampled = resampled.reset_index()
    return resampled


# --- Peer Comparison ---

def _extract_metrics(analysis):
    """Extract key metrics from analysis for comparison"""
    tech = analysis.get('technical', {}) or {}
    fund = analysis.get('fundamental', {}) or {}
    quant = analysis.get('quantitative', {}) or {}
    sent = analysis.get('sentiment', {}) or {}

    tech_m = tech.get('metrics', {}) or {}
    fund_m = fund.get('metrics', {}) or {}
    quant_m = quant.get('metrics', {}) or {}
    sent_m = sent.get('metrics', {}) or {}

    return {
        'signal': analysis.get('overall_signal', 'N/A'),
        'confidence': analysis.get('overall_confidence'),
        'price': analysis.get('current_price'),
        'pe_ratio': fund_m.get('pe_ratio'),
        'roe': fund_m.get('roe'),
        'profit_margin': fund_m.get('profit_margin'),
        'beta': quant_m.get('beta'),
        'volatility': quant_m.get('volatility'),
        'sharpe': quant_m.get('sharpe_ratio'),
        'rsi': tech_m.get('rsi'),
        'sentiment': sent_m.get('normalized_score'),
    }


def _fmt(val, fmt=".2f", suffix=""):
    if val is None:
        return "—"
    try:
        return f"{val:{fmt}}{suffix}"
    except (ValueError, TypeError):
        return str(val)


def _color_best_worst(values, higher_is_better=True):
    """Return list of className strings highlighting best/worst among numeric values"""
    numeric = [(i, v) for i, v in enumerate(values) if v is not None and isinstance(v, (int, float))]
    classes = ["" for _ in values]
    if len(numeric) < 2:
        return classes
    if higher_is_better:
        best_i = max(numeric, key=lambda x: x[1])[0]
        worst_i = min(numeric, key=lambda x: x[1])[0]
    else:
        best_i = min(numeric, key=lambda x: x[1])[0]
        worst_i = max(numeric, key=lambda x: x[1])[0]
    if best_i != worst_i:
        classes[best_i] = "text-success fw-bold"
        classes[worst_i] = "text-danger"
    return classes


def build_comparison_table(symbol, sym_metrics, peer_data):
    """Build a side-by-side comparison table"""
    symbols = [symbol] + list(peer_data.keys())
    all_metrics = [sym_metrics] + [peer_data[p] for p in peer_data]

    rows_config = [
        ("Signal", "signal", None, None),
        ("Confidence", "confidence", ".0%", True),
        ("Price", "price", "$,.2f", None),
        ("P/E Ratio", "pe_ratio", ".1f", None),  # context-dependent
        ("ROE", "roe", ".1f%", True),
        ("Profit Margin", "profit_margin", ".1f%", True),
        ("Beta", "beta", ".2f", None),
        ("Volatility", "volatility", ".1f%", False),
        ("Sharpe Ratio", "sharpe", ".3f", True),
        ("RSI", "rsi", ".1f", None),
        ("Sentiment", "sentiment", "+.1f", True),
    ]

    header = [html.Th("Metric")] + [html.Th(s) for s in symbols]
    table_rows = []

    for label, key, fmt, higher_better in rows_config:
        values = [m.get(key) for m in all_metrics]

        if key == "signal":
            cells = [html.Td(v or "—") for v in values]
        elif key == "confidence":
            formatted = [_fmt(v * 100 if v is not None else None, ".0f", "%") for v in values]
            colors = _color_best_worst(values, True)
            cells = [html.Td(f, className=c) for f, c in zip(formatted, colors)]
        elif key == "price":
            cells = [html.Td(f"${v:,.2f}" if v else "—") for v in values]
        else:
            suffix = ""
            f = ".2f"
            if fmt and "%" in fmt:
                suffix = "%"
                f = fmt.replace("%", "")
            elif fmt:
                f = fmt.lstrip("$,")
            formatted = [_fmt(v, f, suffix) for v in values]
            if higher_better is not None:
                colors = _color_best_worst(values, higher_better)
            else:
                colors = ["" for _ in values]
            cells = [html.Td(f, className=c) for f, c in zip(formatted, colors)]

        table_rows.append(html.Tr([html.Td(html.Strong(label))] + cells))

    return dbc.Table(
        [html.Thead(html.Tr(header))] + [html.Tbody(table_rows)],
        bordered=True,
        hover=True,
        responsive=True,
        className="table-dark",
    )


def build_comparison_radar(symbol, sym_metrics, peer_data):
    """Build radar chart comparing stocks across key dimensions"""
    categories = ["Confidence", "Fundamental", "Risk (inv.)", "Momentum", "Sentiment"]

    def to_radar_values(m):
        conf = (m.get('confidence') or 0.5) * 100
        # Fundamental score: normalize P/E (lower is better, cap at 50) + ROE
        pe = m.get('pe_ratio')
        roe = m.get('roe')
        fund_score = 50
        if pe is not None and pe > 0:
            fund_score = max(0, min(100, 100 - pe * 2))
        if roe is not None:
            fund_score = (fund_score + min(100, max(0, roe * 2))) / 2
        # Risk: invert volatility (lower vol = higher score)
        vol = m.get('volatility')
        risk_score = max(0, min(100, 100 - (vol or 25) * 2))
        # Momentum: RSI normalized (50 = neutral, higher = more bullish)
        rsi = m.get('rsi')
        momentum = rsi if rsi is not None else 50
        # Sentiment: map -100..100 to 0..100
        sent = m.get('sentiment')
        sent_score = ((sent or 0) + 100) / 2
        return [conf, fund_score, risk_score, momentum, sent_score]

    colors = ['#00ccff', '#ff6347', '#00cc88', '#ffaa00', '#cc66ff', '#ff69b4']
    fig = go.Figure()

    all_stocks = [(symbol, sym_metrics)] + list(peer_data.items())
    for i, (sym, metrics) in enumerate(all_stocks):
        vals = to_radar_values(metrics)
        vals_closed = vals + [vals[0]]
        cats_closed = categories + [categories[0]]
        color = colors[i % len(colors)]
        fig.add_trace(go.Scatterpolar(
            r=vals_closed,
            theta=cats_closed,
            fill='toself',
            fillcolor=f"rgba({int(color[1:3],16)},{int(color[3:5],16)},{int(color[5:7],16)},0.1)",
            line=dict(color=color, width=2),
            name=sym,
        ))

    fig.update_layout(
        polar=dict(
            radialaxis=dict(visible=True, range=[0, 100], tickfont=dict(color="#888", size=10), gridcolor="rgba(128,128,128,0.3)"),
            angularaxis=dict(tickfont=dict(color="#fff", size=12), gridcolor="rgba(128,128,128,0.3)"),
            bgcolor="rgba(0,0,0,0)",
        ),
        showlegend=True,
        legend=dict(orientation="h", yanchor="bottom", y=-0.2, xanchor="center", x=0.5, font=dict(color="#fff")),
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font=dict(color="#fff"),
        height=420,
        margin=dict(l=80, r=80, t=40, b=60),
    )
    return fig


# Callback for peer comparison (background + cancel)
@app.callback(
    Output('peer-comparison-section', 'children'),
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_peer_comparison(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return ""

    symbol = symbol.upper()
    peers, sector = get_peers(symbol)

    if not peers:
        return dbc.Card([
            dbc.CardHeader(html.H5("Peer Comparison", className="mb-0")),
            dbc.CardBody(html.P(
                f"No sector peers mapped for {symbol}. Peer comparison is available for major stocks.",
                className="text-muted"
            ))
        ])

    # Try frontend cache for main symbol first
    cached_analysis = _cache_get(f'analysis:{symbol}')
    if cached_analysis:
        sym_metrics = _extract_metrics(cached_analysis)
    else:
        try:
            resp = requests.get(f'{API_BASE_URL}/api/analyze/{symbol}', headers=get_headers(), timeout=30)
            main_data = resp.json()
            if not main_data.get('success'):
                return ""
            sym_metrics = _extract_metrics(main_data['data'])
        except Exception:
            return ""

    # Fetch analysis for all peers in parallel
    def _fetch_peer(peer):
        try:
            resp = requests.get(f'{API_BASE_URL}/api/analyze/{peer}', headers=get_headers(), timeout=30)
            data = resp.json()
            if data.get('success'):
                return peer, _extract_metrics(data['data'])
        except Exception:
            pass
        return peer, None

    peer_metrics = {}
    with ThreadPoolExecutor(max_workers=len(peers)) as executor:
        results = executor.map(_fetch_peer, peers)
        for peer, metrics in results:
            if metrics:
                peer_metrics[peer] = metrics

    if not peer_metrics:
        return dbc.Card([
            dbc.CardHeader(html.H5("Peer Comparison", className="mb-0")),
            dbc.CardBody(html.P("Could not load peer data.", className="text-muted"))
        ])

    table = build_comparison_table(symbol, sym_metrics, peer_metrics)
    radar = build_comparison_radar(symbol, sym_metrics, peer_metrics)

    return dbc.Card([
        dbc.CardHeader([
            html.Div([
                html.H5("Peer Comparison", className="mb-0"),
                dbc.Badge(sector, color="info", className="ms-2") if sector else None,
            ], className="d-flex align-items-center")
        ]),
        dbc.CardBody([
            dbc.Row([
                dbc.Col([table], md=7),
                dbc.Col([dcc.Graph(figure=radar, config={'displayModeBar': False})], md=5),
            ])
        ])
    ])


# Callback for stock suggestions
@app.callback(
    Output('suggestions-display', 'children'),
    Input('suggest-button', 'n_clicks'),
    State('universe-dropdown', 'value'),
)
def update_suggestions(n_clicks, universe):
    if n_clicks == 0:
        return html.P("Click 'Get Suggestions' to see top stock picks based on our analysis",
                     className="text-muted text-center")

    try:
        # Fetch suggestions
        response = requests.get(
            f'{API_BASE_URL}/api/suggest',
            params={'universe': universe, 'limit': 10, 'min_confidence': 0.5},
            headers=get_headers()
        )

        try:
            data = response.json()
        except requests.exceptions.JSONDecodeError:
            return dbc.Alert(f"API Error: {response.text[:200]}", color="danger")

        if not data.get('success'):
            return dbc.Alert(f"Error: {data.get('error', 'Unknown error')}", color="danger")

        result = data['data']
        suggestions = result['suggestions']

        if not suggestions:
            return dbc.Alert(
                f"No stocks matched the criteria. Analyzed {result['total_analyzed']} stocks.",
                color="warning"
            )

        # Create suggestion cards
        suggestion_items = []

        for i, stock in enumerate(suggestions, 1):
            # Signal color
            signal_colors = {
                'StrongBuy': 'success',
                'Buy': 'success',
                'WeakBuy': 'info',
                'Neutral': 'warning',
                'WeakSell': 'warning',
                'Sell': 'danger',
                'StrongSell': 'danger',
            }

            signal = str(stock['signal'])
            color = signal_colors.get(signal, 'secondary')

            suggestion_items.append(
                dbc.Col([
                    dbc.Card([
                        dbc.CardBody([
                            html.H5(f"#{i}. {stock['symbol']}", className="mb-2"),
                            dbc.Badge(signal, color=color, className="mb-2"),
                            html.P(f"Score: {stock['score']:.1f}/100", className="mb-2"),
                            html.P(f"Confidence: {stock['confidence']*100:.0f}%", className="mb-2"),
                            html.Hr(),
                            html.H6("Key Highlights:", className="small"),
                            html.Ul([
                                html.Li(highlight, className="small")
                                for highlight in stock['key_highlights']
                            ]) if stock['key_highlights'] else html.P("No highlights", className="small text-muted"),
                            html.Hr(),
                            dbc.Button(
                                "Analyze",
                                id={'type': 'analyze-suggested', 'index': stock['symbol']},
                                color="primary",
                                size="sm",
                                className="w-100"
                            ),
                        ])
                    ], className="h-100")
                ], md=6, lg=4, className="mb-3")
            )

        return html.Div([
            html.P(
                f"Showing top {len(suggestions)} stocks (analyzed {result['total_analyzed']}, "
                f"{result['total_passed_filters']} passed filters)",
                className="text-muted small mb-3"
            ),
            dbc.Row(suggestion_items)
        ])

    except Exception as e:
        return dbc.Alert(f"Error fetching suggestions: {str(e)}", color="danger")


# Callback to analyze suggested stock when clicked
@app.callback(
    Output('symbol-input', 'value'),
    Input({'type': 'analyze-suggested', 'index': dash.dependencies.ALL}, 'n_clicks'),
    prevent_initial_call=True
)
def analyze_suggested_stock(n_clicks):
    if not any(n_clicks):
        raise dash.exceptions.PreventUpdate

    ctx = dash.callback_context
    if not ctx.triggered:
        raise dash.exceptions.PreventUpdate

    # Get the symbol from the button that was clicked
    button_id = ctx.triggered[0]['prop_id'].split('.')[0]
    import json
    button_data = json.loads(button_id)

    return button_data['index']


# ============================================================================
# NEW DATA ENHANCEMENT CALLBACKS (separate from main callback)
# ============================================================================

@app.callback(
    [Output('earnings-section', 'children'),
     Output('dividend-section', 'children')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_earnings_dividends(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return "", ""
    symbol = symbol.upper()

    # Fetch earnings and dividends in parallel
    with ThreadPoolExecutor(max_workers=2) as executor:
        f_earnings = executor.submit(EarningsPanelComponent.fetch_data, symbol)
        f_dividends = executor.submit(DividendPanelComponent.fetch_data, symbol)

        try:
            earnings_data = f_earnings.result()
            earnings_card = EarningsPanelComponent.create_card(earnings_data, symbol)
        except Exception as e:
            earnings_card = dbc.Card([
                dbc.CardHeader(html.H5("Earnings Analysis", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
        try:
            dividend_data = f_dividends.result()
            dividend_card = DividendPanelComponent.create_card(dividend_data, symbol)
        except Exception as e:
            dividend_card = dbc.Card([
                dbc.CardHeader(html.H5("Dividend Analysis", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
    return earnings_card, dividend_card


@app.callback(
    [Output('options-section', 'children'),
     Output('short-interest-section', 'children')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_options_short_interest(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return "", ""
    symbol = symbol.upper()

    # Fetch options and short interest in parallel
    with ThreadPoolExecutor(max_workers=2) as executor:
        f_options = executor.submit(OptionsFlowComponent.fetch_data, symbol)
        f_short = executor.submit(ShortInterestComponent.fetch_data, symbol)

        try:
            options_data = f_options.result()
            options_card = OptionsFlowComponent.create_card(options_data, symbol)
        except Exception as e:
            options_card = dbc.Card([
                dbc.CardHeader(html.H5("Options Flow", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
        try:
            short_data = f_short.result()
            short_card = ShortInterestComponent.create_card(short_data, symbol)
        except Exception as e:
            short_card = dbc.Card([
                dbc.CardHeader(html.H5("Short Squeeze Risk", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
    return options_card, short_card


@app.callback(
    Output('insider-section', 'children'),
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_insider_activity(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return ""
    symbol = symbol.upper()
    try:
        insider_data = InsiderActivityComponent.fetch_data(symbol)
        return InsiderActivityComponent.create_card(insider_data, symbol)
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Insider Activity", className="mb-0")),
            dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
        ])


@app.callback(
    Output('correlation-section', 'children'),
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_correlation(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return ""
    symbol = symbol.upper()
    try:
        corr_data = CorrelationMatrixComponent.fetch_data(symbol)
        return CorrelationMatrixComponent.create_card(corr_data, symbol)
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Correlation & Beta", className="mb-0")),
            dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
        ])


@app.callback(
    [Output('social-sentiment-section', 'children'),
     Output('macro-overlay-section', 'children')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_social_macro(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return "", ""
    symbol = symbol.upper()

    # Fetch social sentiment, macro indicators, and macro sensitivity in parallel
    with ThreadPoolExecutor(max_workers=3) as executor:
        f_social = executor.submit(SocialSentimentComponent.fetch_data, symbol)
        f_indicators = executor.submit(MacroOverlayComponent.fetch_indicators)
        f_sensitivity = executor.submit(MacroOverlayComponent.fetch_sensitivity, symbol)

        try:
            social_data = f_social.result()
            social_card = SocialSentimentComponent.create_card(social_data, symbol)
        except Exception as e:
            social_card = dbc.Card([
                dbc.CardHeader(html.H5("Social Sentiment", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
        try:
            indicators = f_indicators.result()
            sensitivity = f_sensitivity.result()
            macro_card = MacroOverlayComponent.create_card(indicators, sensitivity, symbol)
        except Exception as e:
            macro_card = dbc.Card([
                dbc.CardHeader(html.H5("Macro Overlay", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
    return social_card, macro_card


# ============================================================================
# ALPHA DECAY & FLOW MAP
# ============================================================================

@app.callback(
    [Output('alpha-decay-section', 'children'),
     Output('flow-map-section', 'children')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_alpha_flow(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return "", ""

    with ThreadPoolExecutor(max_workers=2) as executor:
        f_alpha = executor.submit(AlphaDecayComponent.fetch_all_strategies_health)
        f_flow = executor.submit(FlowMapComponent.fetch_sector_flows)

        # --- Alpha Decay: summary + individual strategy cards ---
        try:
            alpha_data = f_alpha.result()
            if alpha_data and alpha_data.get("strategies"):
                children = [AlphaDecayComponent.create_portfolio_health_card(alpha_data)]
                # Show individual strategy cards with health details
                for s in alpha_data.get("strategies", [])[:6]:
                    children.append(AlphaDecayComponent.create_strategy_card(s))
                alpha_card = html.Div(children)
            elif alpha_data:
                alpha_card = AlphaDecayComponent.create_portfolio_health_card(alpha_data)
            else:
                alpha_card = dbc.Card([
                    dbc.CardHeader(html.H5("Portfolio Strategy Health", className="mb-0")),
                    dbc.CardBody([
                        html.P("No strategies tracked yet.", className="text-muted"),
                        html.Small("Run a backtest to start monitoring strategy health.", className="text-muted fst-italic"),
                    ])
                ])
        except Exception as e:
            alpha_card = dbc.Card([
                dbc.CardHeader(html.H5("Alpha Decay", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])

        # --- Flow Map: summary + sector performance bars + rotation details ---
        try:
            flow_data = f_flow.result()
            if flow_data:
                flow_children = []
                flow_summary = flow_data.get("market_summary")
                if flow_summary:
                    flow_children.append(FlowMapComponent.create_market_summary_card(flow_summary))

                # Sector performance heatmap
                sectors = flow_data.get("sectors", [])
                if sectors:
                    from components.flow_map import create_sector_heatmap
                    heatmap = create_sector_heatmap(sectors)
                    flow_children.append(dcc.Graph(figure=heatmap, config={'displayModeBar': False}))

                # Rotation pattern details
                rotations = flow_data.get("rotations", [])
                if rotations:
                    rot_badges = []
                    for r in rotations:
                        rtype = r.get("rotation_type", "Unknown")
                        conf = r.get("confidence", 0)
                        strength = r.get("strength", 0)
                        gaining = r.get("gaining_sectors", [])
                        losing = r.get("losing_sectors", [])
                        is_bullish = "Growth" in rtype or "Cyclical" in rtype or "Small" in rtype
                        color = "success" if is_bullish else "danger" if conf > 0.5 else "warning"
                        rot_badges.append(
                            dbc.Card(dbc.CardBody([
                                html.Div([
                                    dbc.Badge(rtype, color=color, className="me-2 fs-6"),
                                    html.Small(f"Confidence: {conf*100:.0f}% | Strength: {strength:.0f}", className="text-muted"),
                                ], className="mb-2"),
                                html.Div([
                                    html.Small("Gaining: ", className="text-muted"),
                                    html.Span(", ".join(gaining[:3]) if gaining else "None", className="text-success"),
                                    html.Span(" | ", className="text-muted"),
                                    html.Small("Losing: ", className="text-muted"),
                                    html.Span(", ".join(losing[:3]) if losing else "None", className="text-danger"),
                                ], className="small"),
                            ]), className="mb-2")
                        )
                    flow_children.extend(rot_badges)

                flow_card = html.Div(flow_children) if flow_children else dbc.Card([
                    dbc.CardHeader(html.H5("Flow Map", className="mb-0")),
                    dbc.CardBody(html.P("No data available", className="text-muted"))
                ])
            else:
                flow_card = dbc.Card([
                    dbc.CardHeader(html.H5("Flow Map", className="mb-0")),
                    dbc.CardBody(html.P("No data available", className="text-muted"))
                ])
        except Exception as e:
            flow_card = dbc.Card([
                dbc.CardHeader(html.H5("Flow Map", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
    return alpha_card, flow_card


# ============================================================================
# SMART WATCHLIST & TAX DASHBOARD
# ============================================================================

WATCHLIST_PAGE_SIZE = 4

@app.callback(
    [Output('watchlist-section', 'children'),
     Output('watchlist-store', 'data'),
     Output('watchlist-page', 'data', allow_duplicate=True),
     Output('tax-section', 'children')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True,
)
def update_watchlist_tax(analyze_clicks, refresh_clicks, symbol):
    if not symbol:
        return "", None, 0, ""

    with ThreadPoolExecutor(max_workers=2) as executor:
        f_watchlist = executor.submit(SmartWatchlistComponent.fetch_personalized_feed)
        f_tax = executor.submit(TaxDashboardComponent.fetch_harvest_opportunities)

        # --- Smart Watchlist: summary header (cards rendered by pagination callback) ---
        watchlist_raw = None
        try:
            watchlist_data = f_watchlist.result()
            if watchlist_data:
                watchlist_summary = SmartWatchlistComponent.create_feed_summary(watchlist_data)
                watchlist_raw = watchlist_data.get("opportunities", [])
                watchlist_card = watchlist_summary
            else:
                watchlist_card = dbc.Card([
                    dbc.CardHeader(html.H5("Smart Watchlist", className="mb-0")),
                    dbc.CardBody(html.P("No data available", className="text-muted"))
                ])
        except Exception as e:
            watchlist_card = dbc.Card([
                dbc.CardHeader(html.H5("Smart Watchlist", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])

        # --- Tax Dashboard: summary banner + opportunity cards ---
        try:
            tax_data = f_tax.result()
            if tax_data:
                tax_children = []
                tax_summary = tax_data.get("summary")
                if tax_summary:
                    tax_children.append(TaxDashboardComponent.create_summary_banner(tax_summary))

                opportunities = tax_data.get("opportunities", [])
                if opportunities:
                    for opp in opportunities[:5]:
                        tax_children.append(TaxDashboardComponent.create_opportunity_card(opp))

                tax_card = html.Div(tax_children) if tax_children else dbc.Card([
                    dbc.CardHeader(html.H5("Tax Dashboard", className="mb-0")),
                    dbc.CardBody(html.P("No tax data available", className="text-muted"))
                ])
            else:
                tax_card = dbc.Card([
                    dbc.CardHeader(html.H5("Tax Dashboard", className="mb-0")),
                    dbc.CardBody(html.P("No data available", className="text-muted"))
                ])
        except Exception as e:
            tax_card = dbc.Card([
                dbc.CardHeader(html.H5("Tax Dashboard", className="mb-0")),
                dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
            ])
    return watchlist_card, watchlist_raw, 0, tax_card


@app.callback(
    Output('watchlist-page', 'data'),
    [Input('watchlist-prev', 'n_clicks'),
     Input('watchlist-next', 'n_clicks')],
    [State('watchlist-page', 'data'),
     State('watchlist-store', 'data')],
    prevent_initial_call=True,
)
def change_watchlist_page(prev_clicks, next_clicks, current_page, opportunities):
    if not opportunities:
        return 0
    total_pages = max(1, -(-len(opportunities) // WATCHLIST_PAGE_SIZE))  # ceil div
    triggered = dash.callback_context.triggered[0]['prop_id']
    if 'next' in triggered:
        return min(current_page + 1, total_pages - 1)
    elif 'prev' in triggered:
        return max(current_page - 1, 0)
    return current_page


@app.callback(
    [Output('watchlist-cards', 'children'),
     Output('watchlist-pagination', 'children'),
     Output('watchlist-pagination', 'style')],
    [Input('watchlist-store', 'data'),
     Input('watchlist-page', 'data')],
    prevent_initial_call=True,
)
def render_watchlist_page(opportunities, page):
    if not opportunities:
        return "", "", {'display': 'none'}

    total = len(opportunities)
    total_pages = max(1, -(-total // WATCHLIST_PAGE_SIZE))
    page = min(page or 0, total_pages - 1)

    start = page * WATCHLIST_PAGE_SIZE
    end = start + WATCHLIST_PAGE_SIZE
    page_items = opportunities[start:end]

    cards = []
    for opp in page_items:
        cards.append(SmartWatchlistComponent.create_opportunity_card(opp))

    pagination = html.Div([
        html.Div([
            dbc.Button(
                "Prev", id='watchlist-prev', size="sm",
                color="info", outline=True, className="me-2",
                disabled=(page == 0),
            ),
            html.Span(
                f"Page {page + 1} of {total_pages}  ({total} opportunities)",
                className="small align-self-center mx-2",
                style={"color": "#aaa"},
            ),
            dbc.Button(
                "Next", id='watchlist-next', size="sm",
                color="info", outline=True, className="ms-2",
                disabled=(page >= total_pages - 1),
            ),
        ], className="d-flex justify-content-center align-items-center my-3"),
    ])

    return html.Div(cards), pagination, {'display': 'block'}


# ============================================================================
# TAB SWITCHING
# ============================================================================

@app.callback(
    [Output('tab-trade-content', 'style'),
     Output('tab-portfolio-content', 'style'),
     Output('tab-backtest-content', 'style'),
     Output('tab-live-trade-content', 'style'),
     Output('tab-agent-trades-content', 'style')],
    Input('trading-tabs', 'active_tab'),
)
def switch_trading_tab(active_tab):
    show = {'display': 'block'}
    hide = {'display': 'none'}
    if active_tab == 'tab-trade':
        return show, hide, hide, hide, hide
    elif active_tab == 'tab-portfolio':
        return hide, show, hide, hide, hide
    elif active_tab == 'tab-backtest':
        return hide, hide, show, hide, hide
    elif active_tab == 'tab-live-trade':
        return hide, hide, hide, show, hide
    elif active_tab == 'tab-agent-trades':
        return hide, hide, hide, hide, show
    return show, hide, hide, hide, hide


# ============================================================================
# PAPER TRADING CALLBACKS
# ============================================================================

@app.callback(
    [Output('paper-trading-section', 'children'),
     Output('paper-trade-symbol-store', 'data')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks'),
     Input('paper-trade-notification-store', 'data')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True
)
def update_paper_trading(analyze_clicks, refresh_clicks, notification_data, symbol):
    if not symbol:
        return "", ""
    symbol = symbol.upper()

    try:
        account = PaperTradingComponent.fetch_account()
        position = PaperTradingComponent.fetch_position(symbol)

        # Try to get cached analysis for the signal suggestion
        cached_analysis = _cache_get(f'analysis:{symbol}')

        panel = PaperTradingComponent.create_panel(account, position, symbol, cached_analysis)
        return panel, symbol
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Paper Trading", className="mb-0")),
            dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
        ]), symbol


@app.callback(
    [Output('paper-trade-notification-store', 'data'),
     Output('paper-trade-notification-area', 'children')],
    [Input('paper-trade-buy-btn', 'n_clicks'),
     Input('paper-trade-sell-btn', 'n_clicks')],
    [State('paper-trade-shares', 'value'),
     State('paper-trade-symbol-store', 'data')],
    prevent_initial_call=True
)
def execute_paper_trade(buy_clicks, sell_clicks, shares, symbol):
    if not symbol or not shares:
        raise dash.exceptions.PreventUpdate

    # Guard against spurious fires when panel is dynamically created
    if not buy_clicks and not sell_clicks:
        raise dash.exceptions.PreventUpdate

    ctx = dash.callback_context
    if not ctx.triggered:
        raise dash.exceptions.PreventUpdate

    triggered_id = ctx.triggered[0]['prop_id'].split('.')[0]

    if triggered_id == 'paper-trade-buy-btn':
        action = 'buy'
    elif triggered_id == 'paper-trade-sell-btn':
        action = 'sell'
    else:
        raise dash.exceptions.PreventUpdate

    result = PaperTradingComponent.execute_trade(symbol, action, float(shares))

    if result.get('success'):
        order_data = result.get('data', {})
        status = order_data.get('status', 'submitted')
        alert = dbc.Alert(
            f"Order {action.upper()} {shares} shares of {symbol} - Status: {status}",
            color="success",
            dismissable=True,
            duration=5000,
        )
        notification = {'type': 'success', 'action': action, 'symbol': symbol}
    else:
        error = result.get('error', 'Unknown error')
        alert = dbc.Alert(
            f"Trade failed: {error}",
            color="danger",
            dismissable=True,
            duration=5000,
        )
        notification = {'type': 'error', 'error': error}

    return notification, alert


# ============================================================================
# LIVE TRADING CALLBACKS
# ============================================================================

@app.callback(
    [Output('live-trading-section', 'children'),
     Output('live-trade-symbol-store', 'data')],
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks'),
     Input('live-trade-notification-store', 'data')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True
)
def update_live_trading(analyze_clicks, refresh_clicks, notification_data, symbol):
    if not symbol:
        return "", ""
    symbol = symbol.upper()

    try:
        account = LiveTradingComponent.fetch_account()
        position = LiveTradingComponent.fetch_position(symbol)

        # Try to get cached analysis for the signal suggestion
        cached_analysis = _cache_get(f'analysis:{symbol}')

        panel = LiveTradingComponent.create_panel(account, position, symbol, cached_analysis)
        return panel, symbol
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Live Trading", className="mb-0")),
            dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
        ]), symbol


@app.callback(
    Output('live-trade-buy-btn', 'disabled'),
    Output('live-trade-sell-btn', 'disabled'),
    Input('live-trade-confirm-check', 'value'),
)
def toggle_live_trade_buttons(confirmed):
    disabled = not confirmed
    return disabled, disabled


@app.callback(
    [Output('live-trade-notification-store', 'data'),
     Output('live-trade-notification-area', 'children')],
    [Input('live-trade-buy-btn', 'n_clicks'),
     Input('live-trade-sell-btn', 'n_clicks')],
    [State('live-trade-shares', 'value'),
     State('live-trade-symbol-store', 'data'),
     State('live-trade-confirm-check', 'value')],
    prevent_initial_call=True
)
def execute_live_trade(buy_clicks, sell_clicks, shares, symbol, confirmed):
    if not symbol or not shares:
        raise dash.exceptions.PreventUpdate

    if not confirmed:
        raise dash.exceptions.PreventUpdate

    # Guard against spurious fires when panel is dynamically created
    if not buy_clicks and not sell_clicks:
        raise dash.exceptions.PreventUpdate

    ctx = dash.callback_context
    if not ctx.triggered:
        raise dash.exceptions.PreventUpdate

    triggered_id = ctx.triggered[0]['prop_id'].split('.')[0]

    if triggered_id == 'live-trade-buy-btn':
        action = 'buy'
    elif triggered_id == 'live-trade-sell-btn':
        action = 'sell'
    else:
        raise dash.exceptions.PreventUpdate

    result = LiveTradingComponent.execute_trade(symbol, action, float(shares))

    if result.get('success'):
        order_data = result.get('data', {})
        status = order_data.get('status', 'submitted')
        alert = dbc.Alert(
            f"LIVE Order {action.upper()} {shares} shares of {symbol} - Status: {status}",
            color="success",
            dismissable=True,
            duration=5000,
        )
        notification = {'type': 'success', 'action': action, 'symbol': symbol}
    else:
        error = result.get('error', 'Unknown error')
        alert = dbc.Alert(
            f"Live trade failed: {error}",
            color="danger",
            dismissable=True,
            duration=5000,
        )
        notification = {'type': 'error', 'error': error}

    return notification, alert


# ============================================================================
# AGENT TRADES CALLBACKS
# ============================================================================

@app.callback(
    Output('agent-trades-section', 'children'),
    [Input('trading-tabs', 'active_tab'),
     Input('agent-trade-notification-area', 'children')],
    prevent_initial_call=True
)
def update_agent_trades(active_tab, _notification):
    if active_tab != 'tab-agent-trades':
        raise dash.exceptions.PreventUpdate

    try:
        trades = AgentTradesComponent.fetch_pending_trades()
        return AgentTradesComponent.create_panel(trades)
    except Exception as e:
        return dbc.Alert(f"Error loading agent trades: {e}", color="danger")


@app.callback(
    Output('agent-trade-notification-area', 'children'),
    [Input({'type': 'agent-approve-btn', 'index': dash.ALL}, 'n_clicks'),
     Input({'type': 'agent-reject-btn', 'index': dash.ALL}, 'n_clicks'),
     Input({'type': 'agent-cancel-btn', 'index': dash.ALL}, 'n_clicks'),
     Input({'type': 'agent-close-btn', 'index': dash.ALL}, 'n_clicks')],
    prevent_initial_call=True,
)
def handle_agent_trade_actions(approve_clicks, reject_clicks, cancel_clicks, close_clicks):
    ctx = dash.callback_context
    if not ctx.triggered:
        raise dash.exceptions.PreventUpdate

    triggered = ctx.triggered[0]
    if not triggered['value']:
        raise dash.exceptions.PreventUpdate

    import json
    prop_id = json.loads(triggered['prop_id'].rsplit('.', 1)[0])
    btn_type = prop_id['type']
    index = prop_id['index']

    if btn_type in ('agent-approve-btn', 'agent-reject-btn'):
        action = 'approve' if btn_type == 'agent-approve-btn' else 'reject'
        result = AgentTradesComponent.review_trade(index, action)
        if result.get('success'):
            status = result.get('data', {}).get('status', action + 'd')
            color = 'success' if action == 'approve' else 'secondary'
            return dbc.Alert(f"Trade #{index} {status}.", color=color, dismissable=True, duration=4000)
        else:
            return dbc.Alert(f"Failed to {action} trade #{index}: {result.get('error', 'Unknown')}", color="danger", dismissable=True, duration=5000)

    elif btn_type == 'agent-cancel-btn':
        result = AgentTradesComponent.cancel_order(index)
        if result.get('success'):
            return dbc.Alert(f"Order {index[:8]}... canceled.", color="warning", dismissable=True, duration=4000)
        else:
            return dbc.Alert(f"Cancel failed: {result.get('error', 'Unknown')}", color="danger", dismissable=True, duration=5000)

    elif btn_type == 'agent-close-btn':
        result = AgentTradesComponent.close_position(index)
        if result.get('success'):
            return dbc.Alert(f"Position {index} closed.", color="warning", dismissable=True, duration=4000)
        else:
            return dbc.Alert(f"Close failed: {result.get('error', 'Unknown')}", color="danger", dismissable=True, duration=5000)

    raise dash.exceptions.PreventUpdate


# ============================================================================
# PORTFOLIO ORDER CANCEL / CLOSE CALLBACK
# ============================================================================

@app.callback(
    Output('portfolio-order-notification-area', 'children'),
    [Input({'type': 'order-cancel-btn', 'index': dash.ALL}, 'n_clicks'),
     Input({'type': 'position-close-btn', 'index': dash.ALL}, 'n_clicks')],
    prevent_initial_call=True,
)
def handle_portfolio_order_actions(cancel_clicks, close_clicks):
    ctx = dash.callback_context
    if not ctx.triggered:
        raise dash.exceptions.PreventUpdate

    triggered = ctx.triggered[0]
    if not triggered['value']:
        raise dash.exceptions.PreventUpdate

    import json
    prop_id = json.loads(triggered['prop_id'].rsplit('.', 1)[0])
    btn_type = prop_id['type']
    index = prop_id['index']

    from components.agent_trades import AgentTradesComponent

    if btn_type == 'order-cancel-btn':
        result = AgentTradesComponent.cancel_order(index)
        if result.get('success'):
            return dbc.Alert(f"Order canceled.", color="warning", dismissable=True, duration=4000)
        else:
            return dbc.Alert(f"Cancel failed: {result.get('error', 'Unknown')}", color="danger", dismissable=True, duration=5000)

    elif btn_type == 'position-close-btn':
        result = AgentTradesComponent.close_position(index)
        if result.get('success'):
            return dbc.Alert(f"Position {index} closed.", color="warning", dismissable=True, duration=4000)
        else:
            return dbc.Alert(f"Close failed: {result.get('error', 'Unknown')}", color="danger", dismissable=True, duration=5000)

    raise dash.exceptions.PreventUpdate


# ============================================================================
# PORTFOLIO DASHBOARD CALLBACK
# ============================================================================

@app.callback(
    Output('portfolio-dashboard-section', 'children'),
    [Input('analyze-button', 'n_clicks'),
     Input('refresh-button', 'n_clicks'),
     Input('paper-trade-notification-store', 'data')],
    [State('symbol-input', 'value')],
    prevent_initial_call=True
)
def update_portfolio_dashboard(analyze_clicks, refresh_clicks, notification_data, symbol):
    if not symbol:
        return ""

    try:
        with ThreadPoolExecutor(max_workers=3) as executor:
            f_account = executor.submit(PortfolioDashboardComponent.fetch_account)
            f_positions = executor.submit(PortfolioDashboardComponent.fetch_positions)
            f_orders = executor.submit(PortfolioDashboardComponent.fetch_orders, 20)

            account = f_account.result()
            positions = f_positions.result()
            orders = f_orders.result()

        return PortfolioDashboardComponent.create_dashboard(account, positions, orders)
    except Exception as e:
        return dbc.Card([
            dbc.CardHeader(html.H5("Portfolio Dashboard", className="mb-0")),
            dbc.CardBody(html.P(f"Error: {e}", className="text-muted"))
        ])


# ============================================================================
# BACKTESTING CALLBACK
# ============================================================================

@app.callback(
    [Output('backtest-results-section', 'children'),
     Output('last-backtest-id', 'data'),
     Output('monte-carlo-section', 'children'),
     Output('walk-forward-section', 'children')],
    Input('backtest-run-btn', 'n_clicks'),
    [State('symbol-input', 'value'),
     State('backtest-days-input', 'value')],
    prevent_initial_call=True,
)
def run_backtest(n_clicks, symbol, days):
    if not symbol:
        return "", None, "", ""

    symbol = symbol.upper()
    days = days or 365

    try:
        data = BacktestPanelComponent.fetch_backtest(symbol, days)
        panel = BacktestPanelComponent.create_panel(data, symbol)
        # Extract backtest ID — handle both direct result and wrapped {"backtest": ...} formats
        backtest_id = None
        if data:
            if "backtest" in data and isinstance(data["backtest"], dict):
                backtest_id = data["backtest"].get("id")
            else:
                backtest_id = data.get("id")
        return panel, backtest_id, "", ""
    except Exception as e:
        return dbc.Alert(f"Backtest error: {e}", color="danger"), None, "", ""


@app.callback(
    Output('monte-carlo-section', 'children', allow_duplicate=True),
    Input('monte-carlo-btn', 'n_clicks'),
    State('last-backtest-id', 'data'),
    prevent_initial_call=True,
)
def run_monte_carlo_cb(n_clicks, backtest_id):
    if not backtest_id:
        return dbc.Alert("Run a backtest first, then run Monte Carlo.", color="info")

    try:
        mc_data = BacktestPanelComponent.fetch_monte_carlo(backtest_id)
        return BacktestPanelComponent.create_monte_carlo_panel(mc_data)
    except Exception as e:
        return dbc.Alert(f"Monte Carlo error: {e}", color="danger")


@app.callback(
    Output('walk-forward-section', 'children', allow_duplicate=True),
    Input('walk-forward-btn', 'n_clicks'),
    [State('symbol-input', 'value'),
     State('backtest-days-input', 'value')],
    prevent_initial_call=True,
)
def run_walk_forward_cb(n_clicks, symbol, days):
    if not symbol:
        return ""

    symbol = symbol.upper()
    days = days or 365

    try:
        response = requests.post(
            f"{API_BASE_URL}/api/backtest/walk-forward",
            json={
                "strategy_name": f"WalkForward-{symbol}",
                "symbols": [symbol],
                "start_date": (
                    __import__("datetime").datetime.now()
                    - __import__("datetime").timedelta(days=days)
                ).strftime("%Y-%m-%d"),
                "end_date": __import__("datetime").datetime.now().strftime("%Y-%m-%d"),
                "initial_capital": 10000,
                "position_size_percent": 95,
                "confidence_threshold": 0.5,
            },
            headers=get_headers(),
            timeout=90,
        )
        data = response.json()
        wf_data = data.get("data") if data.get("success") else None
        return BacktestPanelComponent.create_walk_forward_panel(wf_data)
    except Exception as e:
        return dbc.Alert(f"Walk-forward error: {e}", color="danger")


def _kill_port(port):
    """Kill any process still holding the port from a previous run."""
    import signal as _signal
    import subprocess
    try:
        result = subprocess.run(
            ["lsof", "-ti", f":{port}"],
            capture_output=True, text=True, timeout=5,
        )
        pids = result.stdout.strip().split()
        for pid in pids:
            if pid:
                pid_int = int(pid)
                if pid_int != os.getpid():
                    os.kill(pid_int, _signal.SIGTERM)
                    print(f"Killed stale process {pid_int} on port {port}")
    except Exception:
        pass


# ============================================================================
# PAGE NAVIGATION
# ============================================================================

@app.callback(
    [Output('page-dashboard', 'style'),
     Output('page-search', 'style'),
     Output('nav-dashboard', 'active'),
     Output('nav-search', 'active'),
     Output('nav-dashboard', 'style'),
     Output('nav-search', 'style')],
    [Input('nav-dashboard', 'n_clicks'),
     Input('nav-search', 'n_clicks')],
    prevent_initial_call=True,
)
def switch_page(dash_clicks, search_clicks):
    ctx = dash.callback_context
    if not ctx.triggered:
        return ({"display": "block"}, {"display": "none"},
                True, False,
                {"color": "#fff", "fontWeight": "500"}, {"color": "#aaa"})
    triggered_id = ctx.triggered[0]["prop_id"].split(".")[0]
    if triggered_id == "nav-search":
        return ({"display": "none"}, {"display": "block"},
                False, True,
                {"color": "#aaa"}, {"color": "#fff", "fontWeight": "500"})
    return ({"display": "block"}, {"display": "none"},
            True, False,
            {"color": "#fff", "fontWeight": "500"}, {"color": "#aaa"})


# ============================================================================
# SYMBOL SEARCH CALLBACKS
# ============================================================================

@app.callback(
    Output('symbol-search-results', 'children'),
    [Input('symbol-search-btn', 'n_clicks'),
     Input('symbol-search-input', 'n_submit')],
    [State('symbol-search-input', 'value')],
    prevent_initial_call=True,
)
def search_symbols(n_clicks, n_submit, query):
    if not query or len(query.strip()) < 1:
        return ""

    results = SymbolSearchComponent.fetch_search_results(query.strip(), limit=30)
    if not results:
        return dbc.Alert(
            f"No results found for '{query}'. Try a different search term.",
            color="info",
            className="mt-3",
        )

    cards = [html.H5(f"{len(results)} results for '{query}'", className="mb-3")]
    for r in results:
        cards.append(SymbolSearchComponent.create_search_result_card(r))
    return html.Div(cards)


@app.callback(
    Output('symbol-detail-section', 'children'),
    [Input({"type": "search-analyze-btn", "index": dash.ALL}, 'n_clicks'),
     Input({"type": "popular-symbol-btn", "index": dash.ALL}, 'n_clicks')],
    prevent_initial_call=True,
)
def show_symbol_detail(*args):
    ctx = dash.callback_context
    if not ctx.triggered:
        return ""
    # Find which button was clicked
    triggered = ctx.triggered[0]
    if triggered["value"] is None:
        return ""
    prop_id = triggered["prop_id"]
    # Extract symbol from pattern-matching id
    import json
    try:
        id_dict = json.loads(prop_id.split(".")[0])
        symbol = id_dict.get("index", "")
    except (json.JSONDecodeError, AttributeError):
        return ""

    if not symbol:
        return ""

    detail = SymbolSearchComponent.fetch_symbol_detail(symbol)
    if not detail:
        return dbc.Alert(f"Could not load details for {symbol}", color="warning")

    return SymbolSearchComponent.create_symbol_detail_card(detail)


@app.callback(
    Output('symbol-search-input', 'value'),
    Input('symbol-search-clear-btn', 'n_clicks'),
    prevent_initial_call=True,
)
def clear_search(_):
    return ""


@app.callback(
    [Output('nav-dashboard', 'n_clicks', allow_duplicate=True),
     Output('symbol-input', 'value', allow_duplicate=True)],
    Input('search-detail-analyze-btn', 'n_clicks'),
    State('symbol-detail-section', 'children'),
    prevent_initial_call=True,
)
def navigate_to_analyze(n_clicks, detail_children):
    """When user clicks 'Full Analysis' on search detail, switch to dashboard and set symbol"""
    if not n_clicks:
        return dash.no_update, dash.no_update
    # Try to extract the ticker from the detail card
    # The ticker is in the card header as the first H4 element
    try:
        if detail_children and isinstance(detail_children, dict):
            # Walk the children tree to find the ticker
            # The card header H4 contains the ticker text
            header = detail_children.get("props", {}).get("children", [{}])[0]
            header_body = header.get("props", {}).get("children", {})
            if isinstance(header_body, dict):
                h_div = header_body.get("props", {}).get("children", [{}])[0]
                h4 = h_div.get("props", {}).get("children", [{}])[0]
                ticker = h4.get("props", {}).get("children", "")
                if ticker:
                    return 1, ticker
    except Exception:
        pass
    return 1, dash.no_update


if __name__ == '__main__':
    dash_debug = os.environ.get('DASH_DEBUG', 'true').lower() in ('true', '1', 'yes')
    dash_host = os.environ.get('DASH_HOST', '0.0.0.0')
    dash_port = int(os.environ.get('DASH_PORT', '8050'))

    # Clean up stale processes from previous runs
    _kill_port(dash_port)

    print("🚀 Starting InvestIQ Dash Application...")
    print(f"📊 Dashboard will be available at: http://localhost:{dash_port}")
    print("⚠️  Make sure the API server is running on http://localhost:3000")
    app.run_server(debug=dash_debug, host=dash_host, port=dash_port)
