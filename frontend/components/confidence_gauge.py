"""
Confidence Gauge Component

Displays calibrated confidence with uncertainty visualization including:
- Main gauge showing calibrated confidence
- Confidence interval bars
- Uncertainty decomposition (epistemic vs aleatoric)
- Reliability assessment
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class ConfidenceGaugeComponent:
    """Component for displaying calibrated confidence with uncertainty"""

    @staticmethod
    def fetch_calibrated_analysis(symbol: str) -> Optional[Dict]:
        """Fetch calibrated analysis from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/analyze/{symbol}/calibrated",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching calibrated analysis: {e}")
            return None

    @staticmethod
    def fetch_calibration_stats() -> Optional[Dict]:
        """Fetch overall calibration statistics"""
        try:
            response = requests.get(
                f"{API_BASE}/api/calibration/stats",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching calibration stats: {e}")
            return None

    @staticmethod
    def create_reliability_badge(reliability: Dict) -> dbc.Badge:
        """Create a badge showing reliability grade"""
        grade = reliability.get("grade", "?")
        score = reliability.get("score", 0)

        color_map = {
            "A": "success",
            "B": "info",
            "C": "warning",
            "D": "danger",
            "F": "dark",
        }

        return dbc.Badge(
            f"Grade {grade} ({score:.0f})",
            color=color_map.get(grade, "secondary"),
            className="ms-2",
        )

    @staticmethod
    def create_confidence_card(data: Dict, symbol: str) -> dbc.Card:
        """Create a summary card for calibrated confidence"""
        if not data:
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Confidence Compass", className="card-title"),
                        html.P("No calibration data available", className="text-muted"),
                    ]
                ),
                className="mb-3",
            )

        calibrated = data.get("calibrated", {})
        uncertainty = data.get("uncertainty", {})
        original = data.get("original_confidence", 0)

        calibrated_conf = calibrated.get("calibrated_confidence", original)
        lower = calibrated.get("lower_bound", calibrated_conf - 0.1)
        upper = calibrated.get("upper_bound", calibrated_conf + 0.1)
        total_uncertainty = calibrated.get("total_uncertainty", 0.2)

        reliability = uncertainty.get("reliability", {})
        recommendation = calibrated.get("recommendation", "")
        reliability_note = calibrated.get("reliability_note", "")

        # Determine color based on calibrated confidence
        if calibrated_conf >= 0.7:
            color = "success"
        elif calibrated_conf >= 0.5:
            color = "info"
        elif calibrated_conf >= 0.3:
            color = "warning"
        else:
            color = "danger"

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H5(
                                f"Confidence Compass - {symbol}",
                                className="card-title mb-0",
                            ),
                            ConfidenceGaugeComponent.create_reliability_badge(
                                reliability
                            )
                            if reliability
                            else None,
                        ],
                        className="d-flex align-items-center mb-3",
                    ),
                    # Main confidence display
                    html.Div(
                        [
                            html.Div(
                                [
                                    html.Span(
                                        f"{calibrated_conf*100:.0f}%",
                                        className=f"display-4 text-{color}",
                                    ),
                                    html.Small(
                                        " calibrated",
                                        className="text-muted",
                                    ),
                                ],
                                className="mb-2",
                            ),
                            html.Small(
                                f"Raw confidence: {original*100:.0f}%",
                                className="text-muted d-block",
                            ),
                            html.Small(
                                f"95% CI: [{lower*100:.0f}%, {upper*100:.0f}%]",
                                className="text-muted d-block",
                            ),
                        ],
                        className="mb-3",
                    ),
                    # Uncertainty bar
                    html.Div(
                        [
                            html.Small("Uncertainty Level:", className="text-muted"),
                            dbc.Progress(
                                value=total_uncertainty * 100,
                                color=(
                                    "success"
                                    if total_uncertainty < 0.2
                                    else "warning" if total_uncertainty < 0.4 else "danger"
                                ),
                                className="mt-1 mb-2",
                                style={"height": "8px"},
                            ),
                        ]
                    ),
                    # Recommendation
                    html.Div(
                        [
                            html.Small(
                                recommendation,
                                className=(
                                    "text-success"
                                    if "Strong" in recommendation
                                    else "text-warning"
                                    if "caution" in recommendation.lower()
                                    else "text-muted"
                                ),
                            ),
                        ],
                        className="mb-2",
                    ),
                    # Reliability note
                    html.Small(
                        reliability_note,
                        className="text-muted fst-italic",
                    )
                    if reliability_note
                    else None,
                ]
            ),
            className="mb-3",
        )


def create_confidence_gauge(
    calibrated_confidence: float,
    lower_bound: float,
    upper_bound: float,
    original_confidence: Optional[float] = None,
    title: str = "Calibrated Confidence",
) -> go.Figure:
    """
    Create a gauge chart showing calibrated confidence with uncertainty interval

    Args:
        calibrated_confidence: The calibrated confidence value (0-1)
        lower_bound: Lower bound of confidence interval
        upper_bound: Upper bound of confidence interval
        original_confidence: Optional raw confidence for comparison
        title: Chart title

    Returns:
        Plotly figure with gauge
    """
    # Determine color based on confidence level
    if calibrated_confidence >= 0.7:
        gauge_color = "#00cc88"
    elif calibrated_confidence >= 0.5:
        gauge_color = "#00ccff"
    elif calibrated_confidence >= 0.3:
        gauge_color = "#ffaa00"
    else:
        gauge_color = "#ff4444"

    fig = go.Figure()

    # Main gauge
    fig.add_trace(
        go.Indicator(
            mode="gauge+number",
            value=calibrated_confidence * 100,
            domain={"x": [0, 1], "y": [0.3, 1]},
            number={"suffix": "%", "font": {"size": 40, "color": "#ffffff"}},
            gauge={
                "axis": {
                    "range": [0, 100],
                    "tickwidth": 1,
                    "tickcolor": "#888888",
                    "tickfont": {"color": "#888888"},
                },
                "bar": {"color": gauge_color, "thickness": 0.3},
                "bgcolor": "rgba(128, 128, 128, 0.2)",
                "borderwidth": 0,
                "steps": [
                    {"range": [0, 30], "color": "rgba(255, 68, 68, 0.15)"},
                    {"range": [30, 50], "color": "rgba(255, 170, 0, 0.15)"},
                    {"range": [50, 70], "color": "rgba(0, 204, 255, 0.15)"},
                    {"range": [70, 100], "color": "rgba(0, 204, 136, 0.15)"},
                ],
                "threshold": {
                    "line": {"color": "#ffffff", "width": 2},
                    "thickness": 0.8,
                    "value": calibrated_confidence * 100,
                },
            },
        )
    )

    # Confidence interval visualization (bullet chart style below gauge)
    interval_y = 0.15
    bar_height = 0.08

    # Background bar (full range)
    fig.add_shape(
        type="rect",
        x0=0,
        x1=1,
        y0=interval_y - bar_height / 2,
        y1=interval_y + bar_height / 2,
        fillcolor="rgba(128, 128, 128, 0.2)",
        line={"width": 0},
    )

    # Confidence interval bar
    fig.add_shape(
        type="rect",
        x0=lower_bound,
        x1=upper_bound,
        y0=interval_y - bar_height / 2,
        y1=interval_y + bar_height / 2,
        fillcolor="rgba(0, 204, 255, 0.4)",
        line={"color": "#00ccff", "width": 1},
    )

    # Calibrated confidence marker
    fig.add_shape(
        type="line",
        x0=calibrated_confidence,
        x1=calibrated_confidence,
        y0=interval_y - bar_height,
        y1=interval_y + bar_height,
        line={"color": gauge_color, "width": 3},
    )

    # Original confidence marker (if provided)
    if original_confidence is not None and abs(original_confidence - calibrated_confidence) > 0.01:
        fig.add_shape(
            type="line",
            x0=original_confidence,
            x1=original_confidence,
            y0=interval_y - bar_height / 2,
            y1=interval_y + bar_height / 2,
            line={"color": "#ffffff", "width": 2, "dash": "dot"},
        )

        # Add annotation for original
        fig.add_annotation(
            x=original_confidence,
            y=interval_y + bar_height * 1.5,
            text=f"Raw: {original_confidence*100:.0f}%",
            showarrow=False,
            font={"size": 10, "color": "#888888"},
        )

    # Labels for interval
    fig.add_annotation(
        x=lower_bound,
        y=interval_y - bar_height * 1.5,
        text=f"{lower_bound*100:.0f}%",
        showarrow=False,
        font={"size": 10, "color": "#888888"},
    )

    fig.add_annotation(
        x=upper_bound,
        y=interval_y - bar_height * 1.5,
        text=f"{upper_bound*100:.0f}%",
        showarrow=False,
        font={"size": 10, "color": "#888888"},
    )

    fig.add_annotation(
        x=0.5,
        y=interval_y - bar_height * 2.5,
        text="95% Confidence Interval",
        showarrow=False,
        font={"size": 11, "color": "#aaaaaa"},
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": title,
            "font": {"size": 16, "color": "#ffffff"},
            "x": 0.5,
        },
        height=350,
        margin={"l": 40, "r": 40, "t": 60, "b": 40},
        xaxis={"visible": False, "range": [0, 1]},
        yaxis={"visible": False, "range": [0, 1]},
    )

    return fig


def create_uncertainty_breakdown(uncertainty_data: Dict) -> go.Figure:
    """
    Create a visualization of uncertainty decomposition

    Args:
        uncertainty_data: Dictionary with epistemic, aleatoric, and source data

    Returns:
        Plotly figure showing uncertainty breakdown
    """
    decomposition = uncertainty_data.get("uncertainty", {})
    sources = uncertainty_data.get("sources", [])

    epistemic = decomposition.get("epistemic", 0.1)
    aleatoric = decomposition.get("aleatoric", 0.1)
    total = decomposition.get("total", 0.2)

    fig = go.Figure()

    # Stacked bar showing epistemic vs aleatoric
    fig.add_trace(
        go.Bar(
            name="Model (Reducible)",
            x=[epistemic * 100],
            y=["Uncertainty"],
            orientation="h",
            marker_color="#ff8800",
            text=[f"Model: {epistemic*100:.0f}%"],
            textposition="inside",
            textfont={"color": "#ffffff"},
            hovertemplate="Model Uncertainty (Epistemic)<br>%{x:.1f}%<extra></extra>",
        )
    )

    fig.add_trace(
        go.Bar(
            name="Data (Inherent)",
            x=[aleatoric * 100],
            y=["Uncertainty"],
            orientation="h",
            marker_color="#00ccff",
            text=[f"Data: {aleatoric*100:.0f}%"],
            textposition="inside",
            textfont={"color": "#ffffff"},
            hovertemplate="Data Uncertainty (Aleatoric)<br>%{x:.1f}%<extra></extra>",
        )
    )

    # Add source breakdown if available
    if sources:
        y_labels = [s.get("name", "Unknown")[:20] for s in sources]
        x_values = [s.get("contribution", 0) * 100 for s in sources]
        colors = [
            "#ff8800" if s.get("uncertainty_type") == "Epistemic" else "#00ccff"
            for s in sources
        ]

        fig.add_trace(
            go.Bar(
                name="Sources",
                x=x_values,
                y=y_labels,
                orientation="h",
                marker_color=colors,
                text=[f"{v:.0f}%" for v in x_values],
                textposition="outside",
                textfont={"color": "#ffffff", "size": 10},
                hovertemplate="<b>%{y}</b><br>Contribution: %{x:.1f}%<extra></extra>",
                visible="legendonly",
            )
        )

    fig.update_layout(
        barmode="stack",
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": f"Uncertainty Breakdown (Total: {total*100:.0f}%)",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Contribution (%)",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "range": [0, max(50, total * 100 + 10)],
        },
        yaxis={"gridcolor": "rgba(128, 128, 128, 0.2)"},
        legend={
            "orientation": "h",
            "yanchor": "bottom",
            "y": -0.3,
            "xanchor": "center",
            "x": 0.5,
            "font": {"color": "#ffffff"},
        },
        height=200,
        margin={"l": 120, "r": 40, "t": 50, "b": 60},
    )

    return fig


def create_reliability_diagram(bucket_stats: list) -> go.Figure:
    """
    Create a reliability diagram showing calibration quality

    Args:
        bucket_stats: List of bucket statistics with predicted vs actual rates

    Returns:
        Plotly figure with reliability diagram
    """
    if not bucket_stats:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No calibration data available",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=300,
        )
        return fig

    predicted = [b.get("mid_confidence", 0) * 100 for b in bucket_stats]
    actual = [b.get("actual_accuracy", 0) * 100 for b in bucket_stats]
    counts = [b.get("sample_count", 0) for b in bucket_stats]

    fig = go.Figure()

    # Perfect calibration line
    fig.add_trace(
        go.Scatter(
            x=[0, 100],
            y=[0, 100],
            mode="lines",
            name="Perfect Calibration",
            line={"color": "#888888", "dash": "dash"},
        )
    )

    # Actual calibration curve
    fig.add_trace(
        go.Scatter(
            x=predicted,
            y=actual,
            mode="lines+markers",
            name="Actual",
            line={"color": "#00cc88", "width": 2},
            marker={"size": [min(20, 5 + c / 5) for c in counts]},
            hovertemplate="Predicted: %{x:.0f}%<br>Actual: %{y:.0f}%<br>Samples: %{text}<extra></extra>",
            text=counts,
        )
    )

    # Calibration gap shading
    for i in range(len(predicted)):
        if actual[i] < predicted[i]:
            # Overconfident - red shading
            fig.add_shape(
                type="rect",
                x0=predicted[i] - 5,
                x1=predicted[i] + 5,
                y0=actual[i],
                y1=predicted[i],
                fillcolor="rgba(255, 68, 68, 0.2)",
                line={"width": 0},
            )
        else:
            # Underconfident - green shading
            fig.add_shape(
                type="rect",
                x0=predicted[i] - 5,
                x1=predicted[i] + 5,
                y0=predicted[i],
                y1=actual[i],
                fillcolor="rgba(0, 204, 136, 0.2)",
                line={"width": 0},
            )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Reliability Diagram",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Predicted Confidence (%)",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "range": [0, 100],
        },
        yaxis={
            "title": "Actual Accuracy (%)",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "range": [0, 100],
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
        margin={"l": 60, "r": 40, "t": 50, "b": 70},
    )

    return fig
