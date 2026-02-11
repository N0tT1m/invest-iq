"""
Alpha Decay Monitor Component

Visualizes strategy health and performance degradation including:
- Strategy health cards with status badges
- Sharpe ratio trend charts
- CUSUM change detection visualization
- Portfolio health overview
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, List, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class AlphaDecayComponent:
    """Component for monitoring strategy health and alpha decay"""

    @staticmethod
    def fetch_all_strategies_health() -> Optional[Dict]:
        """Fetch health status for all strategies"""
        try:
            response = requests.get(
                f"{API_BASE}/api/strategies/health",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching strategies health: {e}")
            return None

    @staticmethod
    def fetch_strategy_report(strategy_name: str) -> Optional[Dict]:
        """Fetch detailed health report for a strategy"""
        try:
            response = requests.get(
                f"{API_BASE}/api/strategies/{strategy_name}/report",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching strategy report: {e}")
            return None

    @staticmethod
    def fetch_strategy_history(strategy_name: str) -> Optional[List[Dict]]:
        """Fetch performance history for a strategy"""
        try:
            response = requests.get(
                f"{API_BASE}/api/strategies/{strategy_name}/history",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching strategy history: {e}")
            return None

    @staticmethod
    def create_portfolio_health_card(data: Dict) -> dbc.Card:
        """Create overview card for portfolio-wide strategy health"""
        if not data or not data.get("strategies"):
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Portfolio Strategy Health", className="card-title"),
                        html.P("No strategies tracked yet.", className="text-muted"),
                        html.Small(
                            "Run a backtest to automatically create a strategy for monitoring.",
                            className="text-muted fst-italic",
                        ),
                    ]
                ),
                className="mb-3",
            )

        overall_health = data.get("overall_portfolio_health", 0)
        strategies = data.get("strategies", [])

        # Count by status
        status_counts = {}
        for s in strategies:
            status = s.get("status", "Unknown")
            status_counts[status] = status_counts.get(status, 0) + 1

        # Determine overall color
        if overall_health >= 70:
            health_color = "success"
        elif overall_health >= 50:
            health_color = "warning"
        else:
            health_color = "danger"

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H5("Portfolio Strategy Health", className="card-title mb-0"),
                            dbc.Badge(
                                f"{overall_health:.0f}/100",
                                color=health_color,
                                className="ms-2",
                            ),
                        ],
                        className="d-flex align-items-center mb-3",
                    ),
                    # Status distribution
                    html.Div(
                        [
                            html.Span(
                                [
                                    dbc.Badge(
                                        f"{count} {status}",
                                        color=(
                                            "success"
                                            if status == "Healthy"
                                            else "warning"
                                            if status == "Degrading"
                                            else "danger"
                                        ),
                                        className="me-1",
                                    )
                                    for status, count in status_counts.items()
                                ]
                            )
                        ],
                        className="mb-3",
                    ),
                    # Overall progress bar
                    dbc.Progress(
                        value=overall_health,
                        color=health_color,
                        className="mb-2",
                        style={"height": "12px"},
                    ),
                    html.Small(
                        f"Monitoring {len(strategies)} strategies",
                        className="text-muted",
                    ),
                ]
            ),
            className="mb-3",
        )

    @staticmethod
    def create_strategy_card(strategy: Dict) -> dbc.Card:
        """Create a card for a single strategy's health"""
        name = strategy.get("strategy_name", "Unknown")
        status = strategy.get("status", "Unknown")
        status_color = strategy.get("status_color", "#888888")
        health_score = strategy.get("health_score", 0)
        current_sharpe = strategy.get("current_sharpe", 0)
        decay_pct = strategy.get("decay_pct", 0)
        is_decaying = strategy.get("is_decaying", False)
        days_to_breakeven = strategy.get("days_to_breakeven")

        # Card border color based on status
        border_color = (
            "border-success"
            if status == "Healthy"
            else "border-warning" if status == "Degrading" else "border-danger"
        )

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H6(name, className="card-title mb-0"),
                            dbc.Badge(
                                status,
                                style={"backgroundColor": status_color},
                                className="ms-2",
                            ),
                        ],
                        className="d-flex align-items-center mb-2",
                    ),
                    html.Div(
                        [
                            html.Span(
                                f"Sharpe: {current_sharpe:.2f}",
                                className=(
                                    "text-success"
                                    if current_sharpe > 1
                                    else "text-warning"
                                    if current_sharpe > 0
                                    else "text-danger"
                                ),
                            ),
                            html.Span(" | ", className="text-muted"),
                            html.Span(
                                f"Decay: {decay_pct:.1f}%",
                                className="text-danger" if decay_pct > 25 else "text-muted",
                            ),
                        ],
                        className="small mb-2",
                    ),
                    dbc.Progress(
                        value=health_score,
                        color=(
                            "success"
                            if health_score >= 70
                            else "warning" if health_score >= 40 else "danger"
                        ),
                        style={"height": "6px"},
                    ),
                    html.Div(
                        [
                            html.Small(
                                "Decaying" if is_decaying else "Stable",
                                className="text-danger" if is_decaying else "text-success",
                            ),
                            html.Small(
                                f" | ~{days_to_breakeven}d to breakeven"
                                if days_to_breakeven
                                else "",
                                className="text-muted",
                            ),
                        ],
                        className="mt-2",
                    )
                    if is_decaying or days_to_breakeven
                    else None,
                ]
            ),
            className=f"mb-2 {border_color}",
            style={"borderLeft": f"4px solid {status_color}"},
        )


def create_sharpe_trend_chart(
    history: List[Dict],
    strategy_name: str = "Strategy",
) -> go.Figure:
    """
    Create a line chart showing Sharpe ratio trend over time

    Args:
        history: List of performance snapshots
        strategy_name: Name for the chart title

    Returns:
        Plotly figure with trend chart
    """
    if not history:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No historical data available",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=300,
        )
        return fig

    dates = [h.get("snapshot_date") for h in history]
    sharpe = [h.get("rolling_sharpe", 0) for h in history]
    win_rate = [h.get("win_rate", 0) for h in history]

    fig = go.Figure()

    # Sharpe ratio line
    fig.add_trace(
        go.Scatter(
            x=dates,
            y=sharpe,
            mode="lines+markers",
            name="Sharpe Ratio",
            line={"color": "#00cc88", "width": 2},
            marker={"size": 6},
            hovertemplate="<b>%{x}</b><br>Sharpe: %{y:.2f}<extra></extra>",
        )
    )

    # Win rate on secondary axis
    fig.add_trace(
        go.Scatter(
            x=dates,
            y=win_rate,
            mode="lines",
            name="Win Rate (%)",
            line={"color": "#00ccff", "width": 1, "dash": "dot"},
            yaxis="y2",
            hovertemplate="<b>%{x}</b><br>Win Rate: %{y:.1f}%<extra></extra>",
        )
    )

    # Zero line for Sharpe
    fig.add_hline(y=0, line_dash="dash", line_color="#ff4444", opacity=0.5)

    # Trend line
    if len(sharpe) >= 5:
        from numpy import polyfit, poly1d

        x_numeric = list(range(len(sharpe)))
        z = polyfit(x_numeric, sharpe, 1)
        p = poly1d(z)
        trend_color = "#00cc88" if z[0] > 0 else "#ff4444"

        fig.add_trace(
            go.Scatter(
                x=dates,
                y=[p(i) for i in x_numeric],
                mode="lines",
                name="Trend",
                line={"color": trend_color, "width": 2, "dash": "dash"},
                opacity=0.7,
            )
        )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": f"{strategy_name} - Performance Trend",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "title": "Date",
        },
        yaxis={
            "title": "Sharpe Ratio",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
        },
        yaxis2={
            "title": "Win Rate (%)",
            "overlaying": "y",
            "side": "right",
            "range": [0, 100],
            "gridcolor": "rgba(128, 128, 128, 0.1)",
        },
        legend={
            "orientation": "h",
            "yanchor": "bottom",
            "y": -0.25,
            "xanchor": "center",
            "x": 0.5,
            "font": {"color": "#ffffff"},
        },
        height=350,
        margin={"l": 60, "r": 60, "t": 50, "b": 70},
        hovermode="x unified",
    )

    return fig


def create_cusum_chart(cusum_data: Dict, strategy_name: str = "Strategy") -> go.Figure:
    """
    Create a chart showing CUSUM values for change detection

    Args:
        cusum_data: CUSUM analysis result
        strategy_name: Name for the chart title

    Returns:
        Plotly figure with CUSUM visualization
    """
    if not cusum_data:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="Insufficient data for CUSUM analysis",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=250,
        )
        return fig

    upper = cusum_data.get("upper_cusum", [])
    lower = cusum_data.get("lower_cusum", [])
    threshold = cusum_data.get("threshold", 5.0)
    change_points = cusum_data.get("change_points", [])

    x = list(range(len(upper)))

    fig = go.Figure()

    # Upper CUSUM
    fig.add_trace(
        go.Scatter(
            x=x,
            y=upper,
            mode="lines",
            name="Upper CUSUM (Improvement)",
            line={"color": "#00cc88", "width": 1.5},
            fill="tozeroy",
            fillcolor="rgba(0, 204, 136, 0.1)",
        )
    )

    # Lower CUSUM (inverted for visualization)
    fig.add_trace(
        go.Scatter(
            x=x,
            y=[-v for v in lower],
            mode="lines",
            name="Lower CUSUM (Decay)",
            line={"color": "#ff4444", "width": 1.5},
            fill="tozeroy",
            fillcolor="rgba(255, 68, 68, 0.1)",
        )
    )

    # Threshold lines
    fig.add_hline(y=threshold, line_dash="dash", line_color="#ffaa00", opacity=0.7)
    fig.add_hline(y=-threshold, line_dash="dash", line_color="#ffaa00", opacity=0.7)

    # Mark change points
    for cp in change_points:
        idx = cp.get("index", 0)
        direction = cp.get("direction")
        color = "#00cc88" if direction == "Increase" else "#ff4444"

        fig.add_vline(x=idx, line_dash="dot", line_color=color, opacity=0.8)
        fig.add_annotation(
            x=idx,
            y=threshold * 1.2,
            text="Change",
            showarrow=True,
            arrowhead=2,
            arrowcolor=color,
            font={"size": 10, "color": color},
        )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": f"{strategy_name} - Change Detection (CUSUM)",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Time (days)",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
        },
        yaxis={
            "title": "CUSUM Value",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
        },
        legend={
            "orientation": "h",
            "yanchor": "bottom",
            "y": -0.3,
            "xanchor": "center",
            "x": 0.5,
            "font": {"color": "#ffffff"},
        },
        height=280,
        margin={"l": 60, "r": 40, "t": 50, "b": 70},
    )

    return fig


def create_health_score_gauge(health_score: float, strategy_name: str = "") -> go.Figure:
    """
    Create a gauge showing strategy health score

    Args:
        health_score: Health score (0-100)
        strategy_name: Strategy name for title

    Returns:
        Plotly gauge figure
    """
    if health_score >= 70:
        color = "#00cc88"
    elif health_score >= 50:
        color = "#ffaa00"
    elif health_score >= 30:
        color = "#ff6600"
    else:
        color = "#ff4444"

    fig = go.Figure(
        go.Indicator(
            mode="gauge+number",
            value=health_score,
            domain={"x": [0, 1], "y": [0, 1]},
            number={"suffix": "", "font": {"size": 36, "color": "#ffffff"}},
            gauge={
                "axis": {
                    "range": [0, 100],
                    "tickwidth": 1,
                    "tickcolor": "#888888",
                    "tickfont": {"color": "#888888"},
                },
                "bar": {"color": color, "thickness": 0.3},
                "bgcolor": "rgba(128, 128, 128, 0.2)",
                "borderwidth": 0,
                "steps": [
                    {"range": [0, 30], "color": "rgba(255, 68, 68, 0.15)"},
                    {"range": [30, 50], "color": "rgba(255, 102, 0, 0.15)"},
                    {"range": [50, 70], "color": "rgba(255, 170, 0, 0.15)"},
                    {"range": [70, 100], "color": "rgba(0, 204, 136, 0.15)"},
                ],
            },
        )
    )

    title_text = f"{strategy_name} Health" if strategy_name else "Strategy Health"

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": title_text,
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        height=200,
        margin={"l": 30, "r": 30, "t": 50, "b": 20},
    )

    return fig
