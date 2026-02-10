"""
Time Machine - Interactive Historical Replay

A standalone Dash application for learning from historical market events.
Users can step through famous scenarios, make trading decisions, and
compare their performance against AI recommendations.
"""

import dash
from dash import html, dcc, callback, Input, Output, State
import dash_bootstrap_components as dbc
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import requests
import os
from typing import Dict, List, Optional
from datetime import datetime

# API configuration
API_BASE = os.getenv("API_BASE_URL", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "")

def get_headers():
    return {"X-API-Key": API_KEY, "Content-Type": "application/json"}


# Initialize Dash app
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.DARKLY],
    title="Time Machine - Learn from History",
    suppress_callback_exceptions=True,
)

# Color scheme
COLORS = {
    "bg": "#1a1a2e",
    "card": "#16213e",
    "accent": "#0f3460",
    "success": "#00cc88",
    "danger": "#ff6b6b",
    "warning": "#ffd93d",
    "text": "#ffffff",
    "muted": "#888888",
    "buy": "#00cc88",
    "sell": "#ff6b6b",
    "hold": "#ffd93d",
}


def create_difficulty_badge(difficulty: str) -> dbc.Badge:
    """Create a colored badge for difficulty level."""
    color_map = {
        "Beginner": "success",
        "Intermediate": "warning",
        "Advanced": "danger",
        "Expert": "dark",
    }
    return dbc.Badge(
        difficulty,
        color=color_map.get(difficulty, "secondary"),
        className="ms-2",
    )


def create_scenario_card(scenario: Dict) -> dbc.Card:
    """Create a card for a scenario."""
    return dbc.Card(
        dbc.CardBody([
            html.Div([
                html.H5(scenario.get("name", "Unknown"), className="card-title mb-0"),
                create_difficulty_badge(scenario.get("difficulty", "Unknown")),
            ], className="d-flex align-items-center mb-2"),
            html.P(
                scenario.get("description", "")[:150] + "...",
                className="card-text text-muted small mb-2",
            ),
            html.Div([
                html.Small([
                    html.I(className="bi bi-calendar me-1"),
                    f"{scenario.get('start_date', '')} to {scenario.get('end_date', '')}",
                ], className="text-muted me-3"),
                html.Small([
                    html.I(className="bi bi-clock me-1"),
                    f"~{scenario.get('estimated_duration_minutes', 0)} min",
                ], className="text-muted"),
            ], className="mb-3"),
            dbc.Button(
                "Start Scenario",
                id={"type": "start-scenario", "index": scenario.get("id", "")},
                color="primary",
                size="sm",
                className="w-100",
            ),
        ]),
        className="mb-3 bg-dark border-secondary h-100",
    )


def create_price_chart(bars: List[Dict], current_date: str = None) -> go.Figure:
    """Create a candlestick chart with visible history."""
    if not bars:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5, y=0.5,
            text="No data available",
            showarrow=False,
            font={"size": 16, "color": COLORS["muted"]},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0,0,0,0)",
            plot_bgcolor="rgba(0,0,0,0)",
            height=400,
        )
        return fig

    dates = [b["date"] for b in bars]
    opens = [b["open"] for b in bars]
    highs = [b["high"] for b in bars]
    lows = [b["low"] for b in bars]
    closes = [b["close"] for b in bars]

    fig = make_subplots(
        rows=2, cols=1,
        shared_xaxes=True,
        vertical_spacing=0.03,
        row_heights=[0.7, 0.3],
    )

    # Candlestick
    fig.add_trace(
        go.Candlestick(
            x=dates,
            open=opens,
            high=highs,
            low=lows,
            close=closes,
            name="Price",
            increasing_line_color=COLORS["success"],
            decreasing_line_color=COLORS["danger"],
        ),
        row=1, col=1,
    )

    # Volume bars
    volumes = [b.get("volume", 0) for b in bars]
    colors = [COLORS["success"] if closes[i] >= opens[i] else COLORS["danger"]
              for i in range(len(closes))]

    fig.add_trace(
        go.Bar(
            x=dates,
            y=volumes,
            name="Volume",
            marker_color=colors,
            opacity=0.5,
        ),
        row=2, col=1,
    )

    # Highlight current date
    if current_date:
        fig.add_vline(
            x=current_date,
            line_dash="dash",
            line_color=COLORS["warning"],
            annotation_text="Current",
            annotation_position="top",
        )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font={"color": COLORS["text"]},
        xaxis_rangeslider_visible=False,
        showlegend=False,
        height=450,
        margin={"l": 60, "r": 20, "t": 40, "b": 40},
    )

    fig.update_xaxes(gridcolor="rgba(128,128,128,0.2)")
    fig.update_yaxes(gridcolor="rgba(128,128,128,0.2)")

    return fig


def create_indicators_panel(indicators: Dict) -> dbc.Card:
    """Create a panel showing technical indicators."""
    if not indicators:
        return html.Div()

    rsi = indicators.get("rsi_14", 50)
    rsi_color = COLORS["success"] if rsi < 30 else COLORS["danger"] if rsi > 70 else COLORS["text"]

    macd = indicators.get("macd", 0)
    macd_signal = indicators.get("macd_signal", 0)
    macd_color = COLORS["success"] if macd > macd_signal else COLORS["danger"]

    return dbc.Card(
        dbc.CardBody([
            html.H6("Technical Indicators", className="mb-3"),
            html.Div([
                # RSI
                html.Div([
                    html.Small("RSI (14)", className="text-muted"),
                    html.Div([
                        html.Span(f"{rsi:.1f}", style={"color": rsi_color}),
                        html.Small(
                            " Oversold" if rsi < 30 else " Overbought" if rsi > 70 else "",
                            className="text-muted ms-1",
                        ),
                    ]),
                ], className="mb-2"),
                # MACD
                html.Div([
                    html.Small("MACD", className="text-muted"),
                    html.Div([
                        html.Span(f"{macd:.2f}", style={"color": macd_color}),
                        html.Small(f" / Signal: {macd_signal:.2f}", className="text-muted"),
                    ]),
                ], className="mb-2"),
                # Moving Averages
                html.Div([
                    html.Small("SMA 20/50", className="text-muted"),
                    html.Div([
                        html.Span(f"${indicators.get('sma_20', 0):.2f}"),
                        html.Small(" / ", className="text-muted"),
                        html.Span(f"${indicators.get('sma_50', 0):.2f}"),
                    ]),
                ], className="mb-2"),
                # Bollinger Bands
                html.Div([
                    html.Small("Bollinger", className="text-muted"),
                    html.Div([
                        html.Span(f"${indicators.get('bollinger_lower', 0):.2f}"),
                        html.Small(" - ", className="text-muted"),
                        html.Span(f"${indicators.get('bollinger_upper', 0):.2f}"),
                    ]),
                ]),
            ]),
        ]),
        className="bg-dark border-secondary",
    )


def create_portfolio_panel(session: Dict) -> dbc.Card:
    """Create a panel showing current portfolio state."""
    portfolio_value = session.get("portfolio_value", 0)
    starting_capital = session.get("starting_capital", 10000)
    cash = session.get("cash", 0)
    shares = session.get("shares_held", 0)
    cost_basis = session.get("cost_basis", 0)

    total_return = ((portfolio_value - starting_capital) / starting_capital) * 100
    return_color = COLORS["success"] if total_return >= 0 else COLORS["danger"]

    unrealized_pnl = 0
    if shares > 0 and cost_basis > 0:
        # Would need current price to calculate properly
        pass

    return dbc.Card(
        dbc.CardBody([
            html.H6("Portfolio", className="mb-3"),
            html.Div([
                html.Div([
                    html.Small("Portfolio Value", className="text-muted"),
                    html.H4(f"${portfolio_value:,.2f}"),
                ], className="mb-2"),
                html.Div([
                    html.Small("Total Return", className="text-muted"),
                    html.H5(
                        f"{total_return:+.2f}%",
                        style={"color": return_color},
                    ),
                ], className="mb-3"),
                html.Hr(),
                html.Div([
                    html.Small("Cash", className="text-muted d-block"),
                    html.Span(f"${cash:,.2f}"),
                ], className="mb-2"),
                html.Div([
                    html.Small("Shares Held", className="text-muted d-block"),
                    html.Span(f"{shares:,}"),
                ], className="mb-2"),
                html.Div([
                    html.Small("Avg Cost Basis", className="text-muted d-block"),
                    html.Span(f"${cost_basis:.2f}" if cost_basis > 0 else "N/A"),
                ]),
            ]),
        ]),
        className="bg-dark border-secondary",
    )


def create_ai_recommendation_panel(snapshot: Dict) -> dbc.Card:
    """Create a panel showing AI recommendation."""
    if not snapshot:
        return html.Div()

    recommendation = snapshot.get("ai_recommendation", "Hold")
    confidence = snapshot.get("ai_confidence", 0) * 100

    rec_color = {
        "Buy": COLORS["buy"],
        "Sell": COLORS["sell"],
        "Hold": COLORS["hold"],
    }.get(recommendation, COLORS["text"])

    return dbc.Card(
        dbc.CardBody([
            html.H6("AI Recommendation", className="mb-3"),
            html.Div([
                html.H3(
                    recommendation,
                    style={"color": rec_color},
                    className="mb-2",
                ),
                html.Div([
                    html.Small("Confidence: ", className="text-muted"),
                    html.Span(f"{confidence:.0f}%"),
                ]),
                dbc.Progress(
                    value=confidence,
                    color="success" if recommendation == "Buy" else "danger" if recommendation == "Sell" else "warning",
                    className="mt-2",
                    style={"height": "6px"},
                ),
            ]),
        ]),
        className="bg-dark border-secondary",
    )


def create_decision_buttons(session: Dict, snapshot: Dict) -> html.Div:
    """Create trading decision buttons."""
    if not session or session.get("status") != "active":
        return html.Div()

    current_price = snapshot.get("close", 0) if snapshot else 0
    cash = session.get("cash", 0)
    shares = session.get("shares_held", 0)

    max_shares_buyable = int(cash / current_price) if current_price > 0 else 0

    return html.Div([
        html.H6("Make Your Decision", className="mb-3"),
        dbc.Row([
            dbc.Col([
                dbc.Button(
                    [html.I(className="bi bi-arrow-up-circle me-2"), "BUY"],
                    id="btn-buy",
                    color="success",
                    size="lg",
                    className="w-100 mb-2",
                    disabled=max_shares_buyable == 0,
                ),
                html.Small(
                    f"Max: {max_shares_buyable:,} shares",
                    className="text-muted d-block text-center",
                ),
            ], width=4),
            dbc.Col([
                dbc.Button(
                    [html.I(className="bi bi-pause-circle me-2"), "HOLD"],
                    id="btn-hold",
                    color="warning",
                    size="lg",
                    className="w-100 mb-2",
                    outline=True,
                ),
                html.Small(
                    "Skip this day",
                    className="text-muted d-block text-center",
                ),
            ], width=4),
            dbc.Col([
                dbc.Button(
                    [html.I(className="bi bi-arrow-down-circle me-2"), "SELL"],
                    id="btn-sell",
                    color="danger",
                    size="lg",
                    className="w-100 mb-2",
                    disabled=shares == 0,
                ),
                html.Small(
                    f"Holding: {shares:,} shares",
                    className="text-muted d-block text-center",
                ),
            ], width=4),
        ]),
    ], className="mb-4")


def create_decision_history(decisions: List[Dict]) -> dbc.Card:
    """Create a panel showing decision history."""
    if not decisions:
        return html.Div()

    rows = []
    for d in decisions[-5:]:  # Show last 5
        action_color = {
            "Buy": COLORS["buy"],
            "Sell": COLORS["sell"],
            "Hold": COLORS["hold"],
        }.get(str(d.get("action", "Hold")), COLORS["text"])

        actual_return = d.get("actual_return")
        return_text = f"{actual_return:+.1f}%" if actual_return is not None else "N/A"
        return_color = COLORS["success"] if actual_return and actual_return > 0 else COLORS["danger"]

        rows.append(
            html.Tr([
                html.Td(d.get("decision_date", ""), className="small"),
                html.Td(
                    str(d.get("action", "")),
                    style={"color": action_color},
                ),
                html.Td(f"${d.get('price', 0):.2f}", className="small"),
                html.Td(
                    return_text,
                    style={"color": return_color},
                    className="small",
                ),
            ])
        )

    return dbc.Card(
        dbc.CardBody([
            html.H6("Recent Decisions", className="mb-3"),
            dbc.Table(
                [
                    html.Thead(html.Tr([
                        html.Th("Date", className="small"),
                        html.Th("Action", className="small"),
                        html.Th("Price", className="small"),
                        html.Th("Return", className="small"),
                    ])),
                    html.Tbody(rows),
                ],
                size="sm",
                className="mb-0",
            ),
        ]),
        className="bg-dark border-secondary",
    )


def create_score_display(score: Dict) -> html.Div:
    """Create a display for the final score."""
    if not score:
        return html.Div()

    grade = score.get("grade", "?")
    total_points = score.get("total_points", 0)
    user_return = score.get("user_return_pct", 0)
    buy_hold_return = score.get("buy_hold_return_pct", 0)
    accuracy = score.get("accuracy_pct", 0)

    grade_color = {
        "A+": "#00ff88", "A": "#00cc88", "A-": "#00aa66",
        "B+": "#88cc00", "B": "#aacc00", "B-": "#cccc00",
        "C": "#ffcc00", "D": "#ff8800", "F": "#ff4444",
    }.get(grade, COLORS["text"])

    outperformed = user_return > buy_hold_return

    return dbc.Card(
        dbc.CardBody([
            html.Div([
                html.H1(
                    grade,
                    style={"color": grade_color, "fontSize": "5rem"},
                    className="mb-0",
                ),
                html.H4(f"{total_points:,} points", className="text-muted"),
            ], className="text-center mb-4"),

            html.Hr(),

            dbc.Row([
                dbc.Col([
                    html.Div([
                        html.H4(
                            f"{user_return:+.1f}%",
                            style={"color": COLORS["success"] if user_return >= 0 else COLORS["danger"]},
                        ),
                        html.Small("Your Return", className="text-muted"),
                    ], className="text-center"),
                ], width=4),
                dbc.Col([
                    html.Div([
                        html.H4(f"{buy_hold_return:+.1f}%"),
                        html.Small("Buy & Hold", className="text-muted"),
                    ], className="text-center"),
                ], width=4),
                dbc.Col([
                    html.Div([
                        html.H4(f"{accuracy:.0f}%"),
                        html.Small("Accuracy", className="text-muted"),
                    ], className="text-center"),
                ], width=4),
            ], className="mb-4"),

            html.Div([
                dbc.Badge(
                    "Beat Buy & Hold!" if outperformed else "Buy & Hold Won",
                    color="success" if outperformed else "secondary",
                    className="fs-6",
                ),
            ], className="text-center mb-4"),

            html.Div([
                html.H6("Summary"),
                html.P(score.get("summary", ""), className="text-muted"),
            ], className="mb-3"),

            html.Div([
                html.H6("Strengths"),
                html.Ul([
                    html.Li(s, className="text-success small")
                    for s in score.get("strengths", [])
                ]) if score.get("strengths") else html.P("None identified", className="text-muted small"),
            ], className="mb-3"),

            html.Div([
                html.H6("Areas for Improvement"),
                html.Ul([
                    html.Li(s, className="text-warning small")
                    for s in score.get("improvements", [])
                ]) if score.get("improvements") else html.P("None identified", className="text-muted small"),
            ]),
        ]),
        className="bg-dark border-secondary",
    )


# Main layout
app.layout = dbc.Container([
    # Header
    dbc.Row([
        dbc.Col([
            html.H2([
                html.I(className="bi bi-clock-history me-2"),
                "Time Machine",
            ]),
            html.P("Learn from historical market events", className="text-muted"),
        ]),
    ], className="mb-4 mt-3"),

    # Store for session state
    dcc.Store(id="session-store", storage_type="session"),
    dcc.Store(id="scenarios-store", storage_type="memory"),

    # Main content
    html.Div(id="main-content"),

    # Interval for updates (disabled by default)
    dcc.Interval(id="refresh-interval", interval=60000, disabled=True),

], fluid=True, style={"backgroundColor": COLORS["bg"], "minHeight": "100vh"})


@callback(
    Output("scenarios-store", "data"),
    Input("main-content", "id"),  # Trigger on load
)
def load_scenarios(_):
    """Load available scenarios on app start."""
    try:
        response = requests.get(
            f"{API_BASE}/api/time-machine/scenarios",
            headers=get_headers(),
            timeout=30,
        )
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                return data.get("data", [])
    except Exception as e:
        print(f"Error loading scenarios: {e}")
    return []


@callback(
    Output("main-content", "children"),
    Input("session-store", "data"),
    Input("scenarios-store", "data"),
)
def render_main_content(session_data, scenarios):
    """Render main content based on session state."""
    if session_data and session_data.get("session"):
        return render_active_session(session_data)
    else:
        return render_scenario_selection(scenarios or [])


def render_scenario_selection(scenarios: List[Dict]) -> html.Div:
    """Render the scenario selection view."""
    featured = [s for s in scenarios if s.get("is_featured")]
    other = [s for s in scenarios if not s.get("is_featured")]

    return html.Div([
        # Featured scenarios
        html.H4("Featured Scenarios", className="mb-3"),
        dbc.Row([
            dbc.Col(create_scenario_card(s), width=12, md=6, lg=4)
            for s in featured
        ], className="mb-4"),

        # Other scenarios
        html.H4("All Scenarios", className="mb-3") if other else html.Div(),
        dbc.Row([
            dbc.Col(create_scenario_card(s), width=12, md=6, lg=4)
            for s in other
        ]) if other else html.Div(),

        # Custom session option
        html.Hr(className="my-4"),
        html.H5("Or Create Custom Session", className="mb-3"),
        dbc.Row([
            dbc.Col([
                dbc.Input(
                    id="custom-symbol",
                    placeholder="Symbol (e.g., AAPL)",
                    className="mb-2",
                ),
            ], width=12, md=3),
            dbc.Col([
                dbc.Input(
                    id="custom-start-date",
                    type="date",
                    placeholder="Start Date",
                    className="mb-2",
                ),
            ], width=12, md=3),
            dbc.Col([
                dbc.Input(
                    id="custom-end-date",
                    type="date",
                    placeholder="End Date",
                    className="mb-2",
                ),
            ], width=12, md=3),
            dbc.Col([
                dbc.Button(
                    "Start Custom",
                    id="btn-start-custom",
                    color="secondary",
                    className="w-100",
                ),
            ], width=12, md=3),
        ]),
    ])


def render_active_session(session_data: Dict) -> html.Div:
    """Render an active trading session."""
    session = session_data.get("session", {})
    snapshot = session_data.get("current_snapshot")
    score = session_data.get("score")

    status = session.get("status", "active")
    progress = session.get("progress_pct", 0) if hasattr(session, "get") else 0

    # If session is complete, show score
    if status in ["completed", "abandoned"]:
        return html.Div([
            dbc.Row([
                dbc.Col([
                    dbc.Button(
                        [html.I(className="bi bi-arrow-left me-2"), "Back to Scenarios"],
                        id="btn-back",
                        color="secondary",
                        size="sm",
                    ),
                ]),
            ], className="mb-4"),

            dbc.Row([
                dbc.Col([
                    html.H4(f"Session Complete - {session.get('symbol', '')}"),
                    html.P(
                        f"Scenario: {session.get('scenario_id', 'Custom')}",
                        className="text-muted",
                    ),
                ], width=12, md=8),
            ], className="mb-4"),

            dbc.Row([
                dbc.Col([
                    create_score_display(score) if score else html.Div("Loading score..."),
                ], width=12, md=6),
                dbc.Col([
                    dbc.Card(
                        dbc.CardBody([
                            html.H5("Price History"),
                            dcc.Graph(
                                figure=create_price_chart(
                                    snapshot.get("visible_bars", []) if snapshot else [],
                                ),
                                config={"displayModeBar": False},
                            ),
                        ]),
                        className="bg-dark border-secondary",
                    ),
                ], width=12, md=6),
            ]),
        ])

    # Active session
    bars = snapshot.get("visible_bars", []) if snapshot else []
    indicators = snapshot.get("indicators", {}) if snapshot else {}

    return html.Div([
        # Header with progress
        dbc.Row([
            dbc.Col([
                html.Div([
                    html.H4(f"{session.get('symbol', '')} - Time Machine", className="mb-0"),
                    html.Small(
                        f"Day {session.get('days_completed', 0) + 1} of {session.get('total_days', 0)}",
                        className="text-muted",
                    ),
                ]),
            ], width=12, md=8),
            dbc.Col([
                dbc.Button(
                    "Abandon Session",
                    id="btn-abandon",
                    color="outline-danger",
                    size="sm",
                ),
            ], width=12, md=4, className="text-end"),
        ], className="mb-3"),

        # Progress bar
        dbc.Progress(
            value=progress,
            label=f"{progress:.0f}%",
            color="primary",
            className="mb-4",
            style={"height": "8px"},
        ),

        # Main content
        dbc.Row([
            # Left side - Chart and decisions
            dbc.Col([
                # Current date display
                dbc.Alert(
                    [
                        html.H5(
                            f"📅 {snapshot.get('date', '')}" if snapshot else "Loading...",
                            className="mb-0",
                        ),
                        html.Span(
                            f" | Close: ${snapshot.get('close', 0):.2f}" if snapshot else "",
                            className="ms-2",
                        ),
                    ],
                    color="dark",
                    className="mb-3",
                ),

                # Price chart
                dcc.Graph(
                    id="price-chart",
                    figure=create_price_chart(bars, snapshot.get("date") if snapshot else None),
                    config={"displayModeBar": False},
                ),

                # Decision buttons
                html.Div(className="my-4"),
                create_decision_buttons(session, snapshot),

            ], width=12, lg=8),

            # Right side - Info panels
            dbc.Col([
                create_portfolio_panel(session),
                html.Div(className="my-3"),
                create_ai_recommendation_panel(snapshot),
                html.Div(className="my-3"),
                create_indicators_panel(indicators),
                html.Div(className="my-3"),
                create_decision_history(session.get("decisions", [])),
            ], width=12, lg=4),
        ]),
    ])


@callback(
    Output("session-store", "data", allow_duplicate=True),
    Input({"type": "start-scenario", "index": dash.ALL}, "n_clicks"),
    State("scenarios-store", "data"),
    prevent_initial_call=True,
)
def start_scenario(n_clicks, scenarios):
    """Start a scenario when clicked."""
    ctx = dash.callback_context
    if not ctx.triggered or not any(n_clicks):
        return dash.no_update

    # Find which scenario was clicked
    triggered_id = ctx.triggered[0]["prop_id"]
    scenario_id = eval(triggered_id.split(".")[0])["index"]

    try:
        response = requests.post(
            f"{API_BASE}/api/time-machine/start",
            headers=get_headers(),
            json={"scenario_id": scenario_id},
            timeout=30,
        )
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                return data.get("data")
    except Exception as e:
        print(f"Error starting scenario: {e}")

    return dash.no_update


@callback(
    Output("session-store", "data", allow_duplicate=True),
    Input("btn-buy", "n_clicks"),
    Input("btn-hold", "n_clicks"),
    Input("btn-sell", "n_clicks"),
    State("session-store", "data"),
    prevent_initial_call=True,
)
def make_decision(buy_clicks, hold_clicks, sell_clicks, session_data):
    """Handle trading decision buttons."""
    ctx = dash.callback_context
    if not ctx.triggered or not session_data:
        return dash.no_update

    triggered_id = ctx.triggered[0]["prop_id"].split(".")[0]
    action = {
        "btn-buy": "buy",
        "btn-hold": "hold",
        "btn-sell": "sell",
    }.get(triggered_id)

    if not action:
        return dash.no_update

    session = session_data.get("session", {})
    session_id = session.get("id")

    try:
        response = requests.post(
            f"{API_BASE}/api/time-machine/session/{session_id}/decide",
            headers=get_headers(),
            json={"action": action},
            timeout=30,
        )
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                result = data.get("data")
                # If session is complete, get the score
                if result.get("session", {}).get("status") in ["completed", "abandoned"]:
                    try:
                        score_response = requests.get(
                            f"{API_BASE}/api/time-machine/session/{session_id}/score",
                            headers=get_headers(),
                            timeout=30,
                        )
                        if score_response.status_code == 200:
                            score_data = score_response.json()
                            if score_data.get("success"):
                                result["score"] = score_data.get("data")
                    except Exception:
                        pass
                return result
    except Exception as e:
        print(f"Error making decision: {e}")

    return dash.no_update


@callback(
    Output("session-store", "data", allow_duplicate=True),
    Input("btn-abandon", "n_clicks"),
    State("session-store", "data"),
    prevent_initial_call=True,
)
def abandon_session(n_clicks, session_data):
    """Abandon the current session."""
    if not n_clicks or not session_data:
        return dash.no_update

    session = session_data.get("session", {})
    session_id = session.get("id")

    try:
        response = requests.post(
            f"{API_BASE}/api/time-machine/session/{session_id}/abandon",
            headers=get_headers(),
            timeout=30,
        )
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                # Get score for abandoned session
                score_response = requests.get(
                    f"{API_BASE}/api/time-machine/session/{session_id}/score",
                    headers=get_headers(),
                    timeout=30,
                )
                score = None
                if score_response.status_code == 200:
                    score_data = score_response.json()
                    if score_data.get("success"):
                        score = score_data.get("data")

                return {
                    "session": data.get("data"),
                    "current_snapshot": session_data.get("current_snapshot"),
                    "score": score,
                }
    except Exception as e:
        print(f"Error abandoning session: {e}")

    return dash.no_update


@callback(
    Output("session-store", "data", allow_duplicate=True),
    Input("btn-back", "n_clicks"),
    prevent_initial_call=True,
)
def back_to_scenarios(n_clicks):
    """Return to scenario selection."""
    if n_clicks:
        return None
    return dash.no_update


@callback(
    Output("session-store", "data", allow_duplicate=True),
    Input("btn-start-custom", "n_clicks"),
    State("custom-symbol", "value"),
    State("custom-start-date", "value"),
    State("custom-end-date", "value"),
    prevent_initial_call=True,
)
def start_custom_session(n_clicks, symbol, start_date, end_date):
    """Start a custom session."""
    if not n_clicks or not symbol:
        return dash.no_update

    try:
        response = requests.post(
            f"{API_BASE}/api/time-machine/start",
            headers=get_headers(),
            json={
                "symbol": symbol.upper(),
                "start_date": start_date,
                "end_date": end_date,
            },
            timeout=30,
        )
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                return data.get("data")
    except Exception as e:
        print(f"Error starting custom session: {e}")

    return dash.no_update


if __name__ == "__main__":
    app.run(debug=True, port=8053)
