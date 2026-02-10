"""
Smart Watchlist Component

AI-curated, personalized opportunity feed with:
- Opportunity cards ranked by personal relevance
- Signal indicators and confidence badges
- Event countdown for upcoming catalysts
- Quick actions (dismiss, add to watchlist, analyze)
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, List, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class SmartWatchlistComponent:
    """Component for displaying personalized opportunity feed"""

    @staticmethod
    def fetch_personalized_feed(
        user_id: str = "default",
        limit: int = 20,
        min_confidence: float = 0.5,
    ) -> Optional[Dict]:
        """Fetch personalized opportunity feed"""
        try:
            response = requests.get(
                f"{API_BASE}/api/watchlist/personalized",
                params={
                    "user_id": user_id,
                    "limit": limit,
                    "min_confidence": min_confidence,
                },
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching personalized feed: {e}")
            return None

    @staticmethod
    def record_interaction(
        user_id: str,
        symbol: str,
        action: str,
        context: Optional[str] = None,
    ) -> bool:
        """Record user interaction for learning"""
        try:
            response = requests.post(
                f"{API_BASE}/api/watchlist/interaction",
                json={
                    "user_id": user_id,
                    "symbol": symbol,
                    "action": action,
                    "context": context,
                },
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            return response.status_code == 200
        except Exception as e:
            print(f"Error recording interaction: {e}")
            return False

    @staticmethod
    def create_opportunity_card(opportunity: Dict) -> dbc.Card:
        """Create a card for a single opportunity"""
        symbol = opportunity.get("symbol", "???")
        signal = opportunity.get("signal", "Neutral")
        confidence = opportunity.get("confidence", 0.5)
        relevance = opportunity.get("relevance_score", 50)
        reason = opportunity.get("reason", "")
        summary = opportunity.get("summary", "")
        event_type = opportunity.get("event_type")
        event_date = opportunity.get("event_date")
        potential_return = opportunity.get("potential_return")
        sector = opportunity.get("sector")

        # Signal colors
        signal_colors = {
            "StrongBuy": "success",
            "Buy": "info",
            "Neutral": "secondary",
            "Sell": "warning",
            "StrongSell": "danger",
        }
        signal_color = signal_colors.get(signal, "secondary")

        # Relevance color
        if relevance >= 75:
            relevance_color = "success"
        elif relevance >= 50:
            relevance_color = "info"
        else:
            relevance_color = "secondary"

        # Event badge
        event_badge = None
        if event_type:
            days_text = f" ({event_date})" if event_date else ""
            event_badge = dbc.Badge(
                f"{event_type}{days_text}",
                color="warning",
                className="ms-1",
            )

        return dbc.Card(
            [
                dbc.CardHeader(
                    html.Div(
                        [
                            html.Div(
                                [
                                    html.Strong(symbol, className="me-2"),
                                    dbc.Badge(signal, color=signal_color),
                                    event_badge,
                                ],
                                className="d-flex align-items-center",
                            ),
                            html.Div(
                                [
                                    dbc.Badge(
                                        f"{relevance:.0f}% match",
                                        color=relevance_color,
                                        pill=True,
                                    ),
                                ],
                            ),
                        ],
                        className="d-flex justify-content-between align-items-center",
                    )
                ),
                dbc.CardBody(
                    [
                        # Summary
                        html.P(summary, className="small mb-2"),
                        # Metrics row
                        html.Div(
                            [
                                html.Span(
                                    f"Confidence: {confidence*100:.0f}%",
                                    className="small text-muted me-3",
                                ),
                                html.Span(
                                    f"Sector: {sector}",
                                    className="small text-muted me-3",
                                )
                                if sector
                                else None,
                                html.Span(
                                    f"Target: +{potential_return:.1f}%",
                                    className=(
                                        "small text-success"
                                        if potential_return and potential_return > 0
                                        else "small text-danger"
                                    ),
                                )
                                if potential_return
                                else None,
                            ],
                            className="mb-2",
                        ),
                        # Reason
                        html.Small(reason, className="text-muted fst-italic"),
                    ]
                ),
                dbc.CardFooter(
                    html.Div(
                        [
                            dbc.ButtonGroup(
                                [
                                    dbc.Button(
                                        "Analyze",
                                        color="primary",
                                        size="sm",
                                        outline=True,
                                        className="me-1",
                                    ),
                                    dbc.Button(
                                        "+ Watchlist",
                                        color="success",
                                        size="sm",
                                        outline=True,
                                        className="me-1",
                                    ),
                                    dbc.Button(
                                        "Dismiss",
                                        color="secondary",
                                        size="sm",
                                        outline=True,
                                    ),
                                ],
                                size="sm",
                            ),
                        ],
                        className="d-flex justify-content-end",
                    )
                ),
            ],
            className="mb-3",
        )

    @staticmethod
    def create_feed_summary(data: Dict) -> dbc.Card:
        """Create summary card for the feed"""
        if not data:
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Smart Watchlist", className="card-title"),
                        html.P("No opportunities available", className="text-muted"),
                    ]
                ),
                className="mb-3",
            )

        opportunities = data.get("opportunities", [])
        total_scanned = data.get("total_scanned", 0)
        personalized = data.get("user_preferences_applied", False)

        # Count by signal
        signal_counts = {}
        for opp in opportunities:
            signal = opp.get("signal", "Unknown")
            signal_counts[signal] = signal_counts.get(signal, 0) + 1

        # High priority opportunities
        high_priority = [o for o in opportunities if o.get("relevance_score", 0) >= 75]

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H5("Smart Watchlist", className="card-title mb-0"),
                            dbc.Badge(
                                "Personalized" if personalized else "Standard",
                                color="success" if personalized else "secondary",
                                className="ms-2",
                            ),
                        ],
                        className="d-flex align-items-center mb-3",
                    ),
                    html.Div(
                        [
                            html.Span(
                                f"{len(opportunities)} opportunities",
                                className="me-3",
                            ),
                            html.Span(
                                f"({total_scanned} scanned)",
                                className="text-muted me-3",
                            ),
                            html.Span(
                                [
                                    dbc.Badge(
                                        f"{count} {signal}",
                                        color=(
                                            "success"
                                            if "Buy" in signal
                                            else "danger" if "Sell" in signal else "secondary"
                                        ),
                                        className="me-1",
                                    )
                                    for signal, count in signal_counts.items()
                                ]
                            ),
                        ],
                        className="mb-2",
                    ),
                    html.Small(
                        f"{len(high_priority)} high-priority matches",
                        className="text-success",
                    )
                    if high_priority
                    else None,
                ]
            ),
            className="mb-3",
        )


def create_relevance_distribution(opportunities: List[Dict]) -> go.Figure:
    """
    Create a histogram of relevance scores

    Args:
        opportunities: List of opportunities with relevance_score

    Returns:
        Plotly histogram figure
    """
    if not opportunities:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No opportunities to display",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=200,
        )
        return fig

    scores = [o.get("relevance_score", 50) for o in opportunities]

    fig = go.Figure(
        go.Histogram(
            x=scores,
            nbinsx=10,
            marker_color="#00cc88",
            opacity=0.8,
            hovertemplate="Score: %{x:.0f}<br>Count: %{y}<extra></extra>",
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Relevance Score Distribution",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={
            "title": "Relevance Score",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
            "range": [0, 100],
        },
        yaxis={
            "title": "Count",
            "gridcolor": "rgba(128, 128, 128, 0.2)",
        },
        height=200,
        margin={"l": 50, "r": 20, "t": 50, "b": 40},
    )

    return fig


def create_signal_breakdown(opportunities: List[Dict]) -> go.Figure:
    """
    Create a pie chart showing signal distribution

    Args:
        opportunities: List of opportunities

    Returns:
        Plotly pie chart
    """
    if not opportunities:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5,
            y=0.5,
            text="No data",
            showarrow=False,
            font={"size": 14, "color": "#888888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0, 0, 0, 0)",
            plot_bgcolor="rgba(0, 0, 0, 0)",
            height=200,
        )
        return fig

    signal_counts = {}
    for opp in opportunities:
        signal = opp.get("signal", "Unknown")
        signal_counts[signal] = signal_counts.get(signal, 0) + 1

    colors = {
        "StrongBuy": "#00cc88",
        "Buy": "#00ccff",
        "Neutral": "#888888",
        "Sell": "#ffaa00",
        "StrongSell": "#ff4444",
    }

    labels = list(signal_counts.keys())
    values = list(signal_counts.values())
    marker_colors = [colors.get(l, "#888888") for l in labels]

    fig = go.Figure(
        go.Pie(
            labels=labels,
            values=values,
            marker={"colors": marker_colors},
            hole=0.4,
            textfont={"color": "#ffffff"},
            hovertemplate="%{label}: %{value} (%{percent})<extra></extra>",
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font={"color": "#ffffff"},
        title={
            "text": "Signal Breakdown",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        legend={
            "orientation": "h",
            "yanchor": "bottom",
            "y": -0.2,
            "xanchor": "center",
            "x": 0.5,
            "font": {"color": "#ffffff"},
        },
        height=250,
        margin={"l": 20, "r": 20, "t": 50, "b": 60},
    )

    return fig
