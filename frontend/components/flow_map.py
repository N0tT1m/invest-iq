"""
Flow Map Component

Visualizes money flow between sectors using Sankey diagrams and heatmaps:
- Sector performance heatmap
- Sankey diagram showing money flows
- Rotation pattern indicators
- Market sentiment summary
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, List, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class FlowMapComponent:
    """Component for displaying sector flow and rotation analysis"""

    @staticmethod
    def fetch_sector_flows(timeframe: str = "1W") -> Optional[Dict]:
        """Fetch sector flow data"""
        try:
            response = requests.get(
                f"{API_BASE}/api/flows/sectors",
                params={"timeframe": timeframe},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching sector flows: {e}")
            return None

    @staticmethod
    def create_market_summary_card(summary: Dict) -> dbc.Card:
        """Create summary card for market conditions"""
        if not summary:
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Market Flow Summary", className="card-title"),
                        html.P("No data available", className="text-muted"),
                    ]
                ),
                className="mb-3",
            )

        trend = summary.get("trend", "Unknown")
        rotation = summary.get("dominant_rotation")
        strongest = summary.get("strongest_sector", "N/A")
        weakest = summary.get("weakest_sector", "N/A")
        sentiment = summary.get("risk_sentiment", "Neutral")

        # Sentiment color
        sentiment_color = (
            "success"
            if sentiment == "Risk-On"
            else "danger" if sentiment == "Risk-Off" else "secondary"
        )

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H5("Market Flow Summary", className="card-title mb-0"),
                            dbc.Badge(
                                sentiment,
                                color=sentiment_color,
                                className="ms-2",
                            ),
                        ],
                        className="d-flex align-items-center mb-3",
                    ),
                    html.Div(
                        [
                            html.Div(
                                [
                                    html.Small("Trend:", className="text-muted"),
                                    html.Span(f" {trend}", className="ms-1"),
                                ],
                                className="mb-2",
                            ),
                            html.Div(
                                [
                                    html.Small("Rotation:", className="text-muted"),
                                    html.Span(
                                        f" {rotation}" if rotation else " None detected",
                                        className="ms-1",
                                    ),
                                ],
                                className="mb-2",
                            ),
                            html.Div(
                                [
                                    html.Span(
                                        [
                                            html.Small("Leading: ", className="text-muted"),
                                            html.Span(strongest, className="text-success"),
                                        ]
                                    ),
                                    html.Span(" | "),
                                    html.Span(
                                        [
                                            html.Small("Lagging: ", className="text-muted"),
                                            html.Span(weakest, className="text-danger"),
                                        ]
                                    ),
                                ],
                            ),
                        ]
                    ),
                ]
            ),
            className="mb-3",
        )

    @staticmethod
    def create_rotation_badge(rotation: Dict) -> dbc.Badge:
        """Create a badge for a rotation pattern"""
        rotation_type = rotation.get("rotation_type", "None")
        confidence = rotation.get("confidence", 0)
        is_risk_on = "Growth" in rotation_type or "Cyclical" in rotation_type

        color = "success" if is_risk_on else "danger" if confidence > 0.5 else "warning"

        return dbc.Badge(
            f"{rotation_type} ({confidence*100:.0f}%)",
            color=color,
            className="me-1 mb-1",
        )


def create_sector_heatmap(sectors: List[Dict], metric: str = "performance_1w") -> go.Figure:
    """
    Create a heatmap showing sector performance

    Args:
        sectors: List of sector data
        metric: Performance metric to display

    Returns:
        Plotly heatmap figure
    """
    if not sectors:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No sector data available",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=300,
        )
        return fig

    names = [s.get("name", "Unknown") for s in sectors]
    values = [s.get(metric, 0) for s in sectors]

    # Sort by performance
    sorted_data = sorted(zip(names, values), key=lambda x: x[1], reverse=True)
    names, values = zip(*sorted_data)

    # Create color scale (green for positive, red for negative)
    colors = [
        "#00cc88" if v > 1 else "#00aa66" if v > 0 else "#ff6666" if v < -1 else "#ffaa88"
        for v in values
    ]

    fig = go.Figure(
        go.Bar(
            x=list(values),
            y=list(names),
            orientation="h",
            marker={"color": colors},
            text=[f"{v:+.1f}%" for v in values],
            textposition="outside",
            textfont={"color": "#ffffff"},
            hovertemplate="<b>%{y}</b><br>Performance: %{x:+.2f}%<extra></extra>",
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Sector Performance (1W)",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Return (%)",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "zeroline": True,
            "zerolinecolor": "#ffffff",
            "zerolinewidth": 1,
        },
        yaxis={"gridcolor": "rgba(128, 128, 128, 0.2)"},
        height=400,
        margin={"l": 150, "r": 60, "t": 50, "b": 40},
    )

    return fig


def create_flow_sankey(flow_map: Dict) -> go.Figure:
    """
    Create a Sankey diagram showing money flows between sectors

    Args:
        flow_map: Flow map data with sectors and flows

    Returns:
        Plotly Sankey figure
    """
    sectors = flow_map.get("sectors", [])
    flows = flow_map.get("flows", [])

    if not sectors or not flows:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="Insufficient flow data",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=400,
        )
        return fig

    # Create node indices
    sector_names = [s.get("name", f"Sector {i}") for i, s in enumerate(sectors)]
    sector_colors = [s.get("color", "#888888") for s in sectors]
    node_indices = {name: i for i, name in enumerate(sector_names)}

    # Create links
    sources = []
    targets = []
    values = []
    link_colors = []

    for flow in flows:
        from_sector = flow.get("from_sector")
        to_sector = flow.get("to_sector")
        flow_pct = abs(flow.get("flow_percentage", 0))

        if from_sector in node_indices and to_sector in node_indices and flow_pct > 0.5:
            sources.append(node_indices[from_sector])
            targets.append(node_indices[to_sector])
            values.append(flow_pct * 10)  # Scale for visibility
            link_colors.append("rgba(100, 100, 100, 0.4)")

    if not sources:
        # No significant flows, show placeholder
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No significant flows detected",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=400,
        )
        return fig

    fig = go.Figure(
        go.Sankey(
            node={
                "pad": 15,
                "thickness": 20,
                "line": {"color": "rgba(0, 0, 0, 0)", "width": 0},
                "label": sector_names,
                "color": sector_colors,
            },
            link={
                "source": sources,
                "target": targets,
                "value": values,
                "color": link_colors,
            },
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Sector Money Flows",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        height=400,
        margin={"l": 20, "r": 20, "t": 50, "b": 20},
    )

    return fig


def create_rotation_chart(rotations: List[Dict]) -> go.Figure:
    """
    Create a chart showing detected rotation patterns

    Args:
        rotations: List of rotation patterns

    Returns:
        Plotly figure
    """
    if not rotations:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No rotation patterns detected",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=200,
        )
        return fig

    types = [r.get("rotation_type", "Unknown") for r in rotations]
    strengths = [r.get("strength", 0) for r in rotations]
    confidences = [r.get("confidence", 0) for r in rotations]

    colors = ["#00cc88" if "Growth" in t or "Cyclical" in t else "#ff6666" for t in types]

    fig = go.Figure(
        go.Bar(
            x=strengths,
            y=types,
            orientation="h",
            marker={"color": colors},
            text=[f"{c*100:.0f}% conf" for c in confidences],
            textposition="inside",
            textfont={"color": "#ffffff"},
            hovertemplate="<b>%{y}</b><br>Strength: %{x:.1f}<br>Confidence: %{text}<extra></extra>",
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Rotation Patterns",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Strength",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
        },
        yaxis={"gridcolor": "rgba(128, 128, 128, 0.2)"},
        height=200,
        margin={"l": 200, "r": 40, "t": 50, "b": 40},
    )

    return fig


def create_relative_strength_radar(sectors: List[Dict]) -> go.Figure:
    """
    Create a radar chart showing relative strength by sector

    Args:
        sectors: List of sector data

    Returns:
        Plotly radar figure
    """
    if not sectors:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No data available",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=350,
        )
        return fig

    names = [s.get("name", "Unknown") for s in sectors]
    strengths = [s.get("relative_strength", 50) for s in sectors]

    # Close the polygon
    names_closed = names + [names[0]]
    strengths_closed = strengths + [strengths[0]]

    fig = go.Figure()

    fig.add_trace(
        go.Scatterpolar(
            r=strengths_closed,
            theta=names_closed,
            fill="toself",
            fillcolor="rgba(0, 204, 136, 0.3)",
            line={"color": "#00cc88", "width": 2},
            name="Relative Strength",
            hovertemplate="<b>%{theta}</b><br>Strength: %{r:.0f}<extra></extra>",
        )
    )

    # Add 50 (neutral) reference
    fig.add_trace(
        go.Scatterpolar(
            r=[50] * len(names_closed),
            theta=names_closed,
            mode="lines",
            line={"color": "#888888", "width": 1, "dash": "dash"},
            name="Neutral (50)",
        )
    )

    fig.update_layout(
        polar={
            "radialaxis": {
                "visible": True,
                "range": [0, 100],
                "tickfont": {"color": "#888888"},
                "gridcolor": "rgba(128, 128, 128, 0.3)",
            },
            "angularaxis": {
                "tickfont": {"color": "#ffffff", "size": 10},
                "gridcolor": "rgba(128, 128, 128, 0.3)",
            },
            "bgcolor": "rgba(0, 0, 0, 0)",
        },
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Sector Relative Strength",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        showlegend=False,
        height=350,
        margin={"l": 80, "r": 80, "t": 50, "b": 50},
    )

    return fig
