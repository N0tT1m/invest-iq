"""
Sentiment Velocity Component

Visualizes the rate of change in market sentiment with:
- Velocity gauge (speedometer style)
- Trend line with acceleration overlay
- Narrative shift indicators
- Trading signal badges
"""

import plotly.graph_objects as go
from plotly.subplots import make_subplots
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, List, Any, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class SentimentVelocityComponent:
    """Component for displaying sentiment velocity analysis"""

    @staticmethod
    def fetch_velocity_data(symbol: str, days: int = 7) -> Optional[Dict]:
        """Fetch sentiment velocity data from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/sentiment/{symbol}/velocity",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching velocity data: {e}")
            return None

    @staticmethod
    def fetch_history_data(symbol: str, days: int = 30) -> Optional[Dict]:
        """Fetch sentiment history data from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/sentiment/{symbol}/history",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching history data: {e}")
            return None

    @staticmethod
    def create_velocity_card(velocity_data: Optional[Dict], symbol: str) -> dbc.Card:
        """Create a card displaying sentiment velocity information"""
        if not velocity_data:
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Sentiment Velocity", className="card-title"),
                        html.P(
                            "No velocity data available. Analyze the symbol to build history.",
                            className="text-muted",
                        ),
                    ]
                ),
                className="mb-3",
            )

        dynamics = velocity_data.get("dynamics", {})
        current = dynamics.get("current_sentiment", 0)
        velocity = dynamics.get("velocity", 0)
        acceleration = dynamics.get("acceleration", 0)
        signal = dynamics.get("signal", "Stable")
        interpretation = dynamics.get("interpretation", "")
        confidence = dynamics.get("confidence", 0)
        narrative_shift = dynamics.get("narrative_shift")

        # Determine colors based on velocity
        if velocity > 2:
            velocity_color = "success"
            velocity_icon = "bi-arrow-up-circle-fill"
        elif velocity > 0.5:
            velocity_color = "info"
            velocity_icon = "bi-arrow-up-right"
        elif velocity > -0.5:
            velocity_color = "secondary"
            velocity_icon = "bi-dash-circle"
        elif velocity > -2:
            velocity_color = "warning"
            velocity_icon = "bi-arrow-down-right"
        else:
            velocity_color = "danger"
            velocity_icon = "bi-arrow-down-circle-fill"

        # Signal badge color
        signal_colors = {
            "AcceleratingPositive": "success",
            "AcceleratingNegative": "danger",
            "Decelerating": "warning",
            "TurningPoint": "info",
            "Stable": "secondary",
        }
        signal_color = signal_colors.get(signal, "secondary")

        card_content = [
            html.Div(
                [
                    html.H5(
                        [
                            html.I(className=f"bi {velocity_icon} me-2"),
                            "Sentiment Velocity",
                        ],
                        className="card-title mb-3",
                    ),
                    dbc.Badge(signal.replace("_", " "), color=signal_color, className="mb-3"),
                ],
                className="d-flex justify-content-between align-items-center",
            ),
            # Metrics row
            dbc.Row(
                [
                    dbc.Col(
                        [
                            html.Small("Current", className="text-muted d-block"),
                            html.H4(
                                f"{current:.1f}",
                                className=f"text-{'success' if current > 0 else 'danger' if current < 0 else 'secondary'}",
                            ),
                        ],
                        width=4,
                    ),
                    dbc.Col(
                        [
                            html.Small("Velocity", className="text-muted d-block"),
                            html.H4(
                                f"{velocity:+.2f}/day",
                                className=f"text-{velocity_color}",
                            ),
                        ],
                        width=4,
                    ),
                    dbc.Col(
                        [
                            html.Small("Acceleration", className="text-muted d-block"),
                            html.H4(
                                f"{acceleration:+.2f}",
                                className=f"text-{'success' if acceleration > 0 else 'danger' if acceleration < 0 else 'secondary'}",
                            ),
                        ],
                        width=4,
                    ),
                ],
                className="mb-3",
            ),
            # Confidence bar
            html.Div(
                [
                    html.Small(
                        f"Confidence: {confidence * 100:.0f}%", className="text-muted"
                    ),
                    dbc.Progress(
                        value=confidence * 100,
                        color=velocity_color,
                        className="mt-1",
                        style={"height": "6px"},
                    ),
                ],
                className="mb-3",
            ),
            # Interpretation
            html.P(interpretation, className="small text-muted mb-2"),
        ]

        # Add narrative shift if detected
        if narrative_shift:
            card_content.append(
                dbc.Alert(
                    [
                        html.Strong("Narrative Shift Detected: "),
                        f"{narrative_shift.get('from_theme', '?')} → {narrative_shift.get('to_theme', '?')}",
                        html.Small(
                            f" ({narrative_shift.get('confidence', 0) * 100:.0f}% confidence)",
                            className="ms-2",
                        ),
                    ],
                    color="info",
                    className="mb-0 mt-2 py-2",
                )
            )

        return dbc.Card(dbc.CardBody(card_content), className="mb-3")


def create_velocity_gauge(
    velocity: float,
    current_sentiment: float,
    signal: str,
    title: str = "Sentiment Velocity",
) -> go.Figure:
    """
    Create a gauge chart showing sentiment velocity

    Args:
        velocity: Rate of change in sentiment per day
        current_sentiment: Current sentiment score (-100 to 100)
        signal: Velocity signal type
        title: Chart title

    Returns:
        Plotly figure with gauge visualization
    """
    # Normalize velocity to gauge range (-10 to 10)
    gauge_value = max(min(velocity, 10), -10)

    # Determine color based on velocity
    if velocity > 2:
        bar_color = "#00ff00"
    elif velocity > 0.5:
        bar_color = "#00cc88"
    elif velocity > -0.5:
        bar_color = "#888888"
    elif velocity > -2:
        bar_color = "#ff8800"
    else:
        bar_color = "#ff0000"

    fig = go.Figure(
        go.Indicator(
            mode="gauge+number+delta",
            value=gauge_value,
            domain={"x": [0, 1], "y": [0, 1]},
            title={"text": title, "font": {"size": 16, "color": "#ffffff"}},
            delta={
                "reference": 0,
                "increasing": {"color": "#00ff00"},
                "decreasing": {"color": "#ff0000"},
            },
            number={
                "suffix": "/day",
                "font": {"size": 24, "color": "#ffffff"},
            },
            gauge={
                "axis": {
                    "range": [-10, 10],
                    "tickwidth": 1,
                    "tickcolor": "#ffffff",
                    "tickfont": {"color": "#ffffff"},
                },
                "bar": {"color": bar_color},
                "bgcolor": "rgba(0,0,0,0)",
                "borderwidth": 2,
                "bordercolor": "#444444",
                "steps": [
                    {"range": [-10, -5], "color": "rgba(255, 0, 0, 0.3)"},
                    {"range": [-5, -2], "color": "rgba(255, 136, 0, 0.3)"},
                    {"range": [-2, 2], "color": "rgba(128, 128, 128, 0.3)"},
                    {"range": [2, 5], "color": "rgba(0, 204, 136, 0.3)"},
                    {"range": [5, 10], "color": "rgba(0, 255, 0, 0.3)"},
                ],
                "threshold": {
                    "line": {"color": "#ffffff", "width": 2},
                    "thickness": 0.75,
                    "value": gauge_value,
                },
            },
        )
    )

    # Add annotation for current sentiment
    fig.add_annotation(
        x=0.5,
        y=-0.15,
        xref="paper",
        yref="paper",
        text=f"Current Sentiment: {current_sentiment:.1f} | Signal: {signal}",
        showarrow=False,
        font={"size": 12, "color": "#aaaaaa"},
    )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font={"color": "#ffffff"},
        height=300,
        margin=dict(l=20, r=20, t=60, b=40),
    )

    return fig


def create_sentiment_history_chart(
    history: List[Dict],
    symbol: str,
) -> go.Figure:
    """
    Create a chart showing sentiment history with velocity overlay

    Args:
        history: List of sentiment history points
        symbol: Stock symbol

    Returns:
        Plotly figure with sentiment history
    """
    if not history:
        fig = go.Figure()
        fig.add_annotation(
            text="No sentiment history available",
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
            font={"size": 16, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0,0,0,0)",
            plot_bgcolor="rgba(0,0,0,0)",
            height=300,
        )
        return fig

    timestamps = [h.get("timestamp", "") for h in history]
    scores = [h.get("sentiment_score", 0) for h in history]
    article_counts = [h.get("article_count", 0) for h in history]

    fig = make_subplots(
        rows=2,
        cols=1,
        shared_xaxes=True,
        vertical_spacing=0.1,
        row_heights=[0.7, 0.3],
        subplot_titles=(f"{symbol} Sentiment Score", "Article Count"),
    )

    # Sentiment line
    fig.add_trace(
        go.Scatter(
            x=timestamps,
            y=scores,
            mode="lines+markers",
            name="Sentiment",
            line=dict(color="#00ccff", width=2),
            marker=dict(size=6),
            fill="tozeroy",
            fillcolor="rgba(0, 204, 255, 0.1)",
        ),
        row=1,
        col=1,
    )

    # Zero line
    fig.add_hline(
        y=0, line_dash="dash", line_color="#666666", row=1, col=1
    )

    # Overbought/oversold zones
    fig.add_hrect(
        y0=50, y1=100, fillcolor="rgba(0, 255, 0, 0.1)", line_width=0, row=1, col=1
    )
    fig.add_hrect(
        y0=-100, y1=-50, fillcolor="rgba(255, 0, 0, 0.1)", line_width=0, row=1, col=1
    )

    # Article count bars
    colors = ["#00ff00" if s > 0 else "#ff0000" if s < 0 else "#888888" for s in scores]
    fig.add_trace(
        go.Bar(
            x=timestamps,
            y=article_counts,
            name="Articles",
            marker_color=colors,
            opacity=0.7,
        ),
        row=2,
        col=1,
    )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font={"color": "#ffffff"},
        height=400,
        margin=dict(l=60, r=20, t=40, b=20),
        showlegend=False,
        hovermode="x unified",
    )

    fig.update_xaxes(gridcolor="rgba(128,128,128,0.2)", showgrid=True)
    fig.update_yaxes(
        gridcolor="rgba(128,128,128,0.2)",
        showgrid=True,
        range=[-100, 100],
        row=1,
        col=1,
    )
    fig.update_yaxes(gridcolor="rgba(128,128,128,0.2)", showgrid=True, row=2, col=1)

    return fig
