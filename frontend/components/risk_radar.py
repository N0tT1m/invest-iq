"""
Risk Radar Component

Multi-dimensional risk visualization using a radar/spider chart showing:
- Market Risk (beta, correlation)
- Volatility Risk (ATR, realized vol)
- Liquidity Risk (bid-ask, volume)
- Event Risk (earnings, dividends)
- Concentration Risk (position size, sector exposure)
- Sentiment Risk (news volatility)
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, List, Any, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


class RiskRadarComponent:
    """Component for displaying multi-dimensional risk analysis"""

    # Risk dimension labels
    DIMENSIONS = [
        "Market Risk",
        "Volatility",
        "Liquidity",
        "Event Risk",
        "Concentration",
        "Sentiment",
    ]

    # Descriptions for each dimension
    DIMENSION_INFO = {
        "Market Risk": "Correlation with broader market (beta, SPY correlation)",
        "Volatility": "Price volatility (ATR, realized volatility)",
        "Liquidity": "Ease of trading (bid-ask spread, volume)",
        "Event Risk": "Upcoming events (earnings, dividends, FDA)",
        "Concentration": "Position size relative to portfolio",
        "Sentiment": "News sentiment volatility and uncertainty",
    }

    @staticmethod
    def fetch_risk_radar(symbol: Optional[str] = None) -> Optional[Dict]:
        """Fetch risk radar data from API"""
        try:
            endpoint = f"/api/risk/radar/{symbol}" if symbol else "/api/risk/radar"
            response = requests.get(
                f"{API_BASE}{endpoint}",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching risk radar: {e}")
            return None

    @staticmethod
    def calculate_risk_from_analysis(analysis: Dict) -> Dict[str, float]:
        """
        Calculate risk scores from existing analysis data

        Args:
            analysis: Unified analysis response from /api/analyze

        Returns:
            Dictionary with risk scores (0-100) for each dimension
        """
        risk_scores = {
            "market_risk": 50.0,
            "volatility_risk": 50.0,
            "liquidity_risk": 30.0,
            "event_risk": 30.0,
            "concentration_risk": 50.0,
            "sentiment_risk": 50.0,
        }

        # Extract from quantitative analysis
        quant = analysis.get("quantitative") or analysis.get("quant") or {}
        if quant:
            metrics = quant.get("metrics", {})

            # Market risk from beta
            beta = metrics.get("beta", 1.0)
            if isinstance(beta, (int, float)):
                # Beta > 1.5 is high risk, < 0.5 is low
                risk_scores["market_risk"] = min(100, max(0, beta * 50))

            # Volatility risk
            volatility = metrics.get("volatility", 0.2)
            if isinstance(volatility, (int, float)):
                # Annualized vol > 50% is very high risk
                risk_scores["volatility_risk"] = min(100, max(0, volatility * 200))

            # Max drawdown contributes to volatility risk
            max_dd = metrics.get("max_drawdown", 0)
            if isinstance(max_dd, (int, float)):
                risk_scores["volatility_risk"] = min(100, max(0,
                    risk_scores["volatility_risk"] * 0.6 + abs(max_dd) * 100 * 0.4
                ))

        # Sentiment risk from sentiment analysis
        sentiment = analysis.get("sentiment", {})
        if sentiment:
            metrics = sentiment.get("metrics", {})
            confidence = sentiment.get("confidence", 0.5)

            # Low confidence = high uncertainty = high sentiment risk
            risk_scores["sentiment_risk"] = min(100, max(0, (1 - confidence) * 100))

            # Check article distribution
            positive = metrics.get("positive_articles", 0)
            negative = metrics.get("negative_articles", 0)
            total = metrics.get("total_articles", 1)

            if total > 0:
                # Mixed sentiment = higher risk
                balance = abs(positive - negative) / total
                risk_scores["sentiment_risk"] = min(100, max(0,
                    risk_scores["sentiment_risk"] * 0.5 + (1 - balance) * 100 * 0.5
                ))

        # Technical analysis contribution
        technical = analysis.get("technical", {})
        if technical:
            confidence = technical.get("confidence", 0.5)
            # Lower confidence = higher event/uncertainty risk
            risk_scores["event_risk"] = min(100, max(0, (1 - confidence) * 60 + 20))

        # Ensure all scores are clamped to 0-100
        return {k: min(100.0, max(0.0, v)) for k, v in risk_scores.items()}

    @staticmethod
    def create_risk_card(risk_data: Dict, symbol: Optional[str] = None) -> dbc.Card:
        """Create a summary card for risk metrics"""
        if not risk_data:
            return dbc.Card(
                dbc.CardBody(
                    [
                        html.H5("Risk Radar", className="card-title"),
                        html.P("No risk data available", className="text-muted"),
                    ]
                ),
                className="mb-3",
            )

        # Calculate overall risk score
        risk_values = [
            risk_data.get("market_risk", 50),
            risk_data.get("volatility_risk", 50),
            risk_data.get("liquidity_risk", 30),
            risk_data.get("event_risk", 30),
            risk_data.get("concentration_risk", 50),
            risk_data.get("sentiment_risk", 50),
        ]
        avg_risk = sum(risk_values) / len(risk_values)

        # Determine risk level
        if avg_risk < 30:
            risk_level = "Low"
            risk_color = "success"
        elif avg_risk < 50:
            risk_level = "Moderate"
            risk_color = "info"
        elif avg_risk < 70:
            risk_level = "Elevated"
            risk_color = "warning"
        else:
            risk_level = "High"
            risk_color = "danger"

        title = f"Risk Radar - {symbol}" if symbol else "Portfolio Risk Radar"

        # Find highest risk dimensions
        risk_items = [
            ("Market", risk_data.get("market_risk", 50)),
            ("Volatility", risk_data.get("volatility_risk", 50)),
            ("Liquidity", risk_data.get("liquidity_risk", 30)),
            ("Event", risk_data.get("event_risk", 30)),
            ("Concentration", risk_data.get("concentration_risk", 50)),
            ("Sentiment", risk_data.get("sentiment_risk", 50)),
        ]
        sorted_risks = sorted(risk_items, key=lambda x: x[1], reverse=True)
        top_risks = sorted_risks[:2]

        return dbc.Card(
            dbc.CardBody(
                [
                    html.Div(
                        [
                            html.H5(title, className="card-title mb-0"),
                            dbc.Badge(
                                f"{risk_level} Risk",
                                color=risk_color,
                                className="ms-2",
                            ),
                        ],
                        className="d-flex align-items-center mb-3",
                    ),
                    # Overall risk gauge
                    html.Div(
                        [
                            html.Small(
                                f"Overall Risk Score: {avg_risk:.0f}/100",
                                className="text-muted",
                            ),
                            dbc.Progress(
                                value=avg_risk,
                                color=risk_color,
                                className="mt-1 mb-3",
                                style={"height": "10px"},
                            ),
                        ]
                    ),
                    # Top risk factors
                    html.Div(
                        [
                            html.Small("Top Risk Factors:", className="text-muted"),
                            html.Ul(
                                [
                                    html.Li(
                                        f"{name}: {score:.0f}",
                                        className=f"text-{'danger' if score > 60 else 'warning' if score > 40 else 'success'}",
                                    )
                                    for name, score in top_risks
                                ],
                                className="mb-0 ps-3",
                            ),
                        ]
                    ),
                ]
            ),
            className="mb-3",
        )


def create_risk_radar_chart(
    current_risk: Dict[str, float],
    target_risk: Optional[Dict[str, float]] = None,
    title: str = "Risk Radar",
) -> go.Figure:
    """
    Create a radar/spider chart for multi-dimensional risk visualization

    Args:
        current_risk: Current risk scores for each dimension (0-100)
        target_risk: Optional target risk profile for comparison
        title: Chart title

    Returns:
        Plotly figure with radar chart
    """
    categories = [
        "Market Risk",
        "Volatility",
        "Liquidity",
        "Event Risk",
        "Concentration",
        "Sentiment",
    ]

    # Extract values in order
    current_values = [
        current_risk.get("market_risk", 50),
        current_risk.get("volatility_risk", 50),
        current_risk.get("liquidity_risk", 30),
        current_risk.get("event_risk", 30),
        current_risk.get("concentration_risk", 50),
        current_risk.get("sentiment_risk", 50),
    ]

    # Close the polygon
    current_values_closed = current_values + [current_values[0]]
    categories_closed = categories + [categories[0]]

    fig = go.Figure()

    # Current risk profile
    fig.add_trace(
        go.Scatterpolar(
            r=current_values_closed,
            theta=categories_closed,
            fill="toself",
            fillcolor="rgba(255, 99, 71, 0.3)",
            line=dict(color="#ff6347", width=2),
            name="Current Risk",
            hovertemplate="<b>%{theta}</b><br>Risk: %{r:.0f}/100<extra></extra>",
        )
    )

    # Target risk profile (if provided)
    if target_risk:
        target_values = [
            target_risk.get("market_risk", 50),
            target_risk.get("volatility_risk", 50),
            target_risk.get("liquidity_risk", 30),
            target_risk.get("event_risk", 30),
            target_risk.get("concentration_risk", 50),
            target_risk.get("sentiment_risk", 50),
        ]
        target_values_closed = target_values + [target_values[0]]

        fig.add_trace(
            go.Scatterpolar(
                r=target_values_closed,
                theta=categories_closed,
                fill="toself",
                fillcolor="rgba(0, 204, 136, 0.2)",
                line=dict(color="#00cc88", width=2, dash="dash"),
                name="Target Profile",
                hovertemplate="<b>%{theta}</b><br>Target: %{r:.0f}/100<extra></extra>",
            )
        )

    fig.update_layout(
        polar=dict(
            radialaxis=dict(
                visible=True,
                range=[0, 100],
                tickfont=dict(color="#888888", size=10),
                gridcolor="rgba(128, 128, 128, 0.3)",
            ),
            angularaxis=dict(
                tickfont=dict(color="#ffffff", size=12),
                gridcolor="rgba(128, 128, 128, 0.3)",
            ),
            bgcolor="rgba(0, 0, 0, 0)",
        ),
        showlegend=True,
        legend=dict(
            orientation="h",
            yanchor="bottom",
            y=-0.2,
            xanchor="center",
            x=0.5,
            font=dict(color="#ffffff"),
        ),
        paper_bgcolor="rgba(0, 0, 0, 0)",
        plot_bgcolor="rgba(0, 0, 0, 0)",
        font=dict(color="#ffffff"),
        title=dict(
            text=title,
            font=dict(size=16, color="#ffffff"),
            x=0.5,
        ),
        height=400,
        margin=dict(l=80, r=80, t=60, b=60),
    )

    return fig


def create_risk_breakdown_bars(risk_data: Dict[str, float]) -> go.Figure:
    """
    Create a horizontal bar chart showing risk breakdown by dimension

    Args:
        risk_data: Risk scores for each dimension

    Returns:
        Plotly figure with bar chart
    """
    dimensions = [
        "Market Risk",
        "Volatility",
        "Liquidity",
        "Event Risk",
        "Concentration",
        "Sentiment",
    ]

    values = [
        risk_data.get("market_risk", 50),
        risk_data.get("volatility_risk", 50),
        risk_data.get("liquidity_risk", 30),
        risk_data.get("event_risk", 30),
        risk_data.get("concentration_risk", 50),
        risk_data.get("sentiment_risk", 50),
    ]

    # Color based on risk level
    colors = []
    for v in values:
        if v < 30:
            colors.append("#00cc88")
        elif v < 50:
            colors.append("#00ccff")
        elif v < 70:
            colors.append("#ffaa00")
        else:
            colors.append("#ff4444")

    fig = go.Figure(
        go.Bar(
            x=values,
            y=dimensions,
            orientation="h",
            marker_color=colors,
            text=[f"{v:.0f}" for v in values],
            textposition="outside",
            textfont=dict(color="#ffffff"),
            hovertemplate="<b>%{y}</b><br>Risk Score: %{x:.0f}/100<extra></extra>",
        )
    )

    # Add risk zone annotations
    fig.add_vline(x=30, line_dash="dash", line_color="#00cc88", opacity=0.5)
    fig.add_vline(x=50, line_dash="dash", line_color="#ffaa00", opacity=0.5)
    fig.add_vline(x=70, line_dash="dash", line_color="#ff4444", opacity=0.5)

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font=dict(color="#ffffff"),
        xaxis=dict(
            range=[0, 100],
            gridcolor="rgba(128,128,128,0.2)",
            title="Risk Score",
        ),
        yaxis=dict(
            gridcolor="rgba(128,128,128,0.2)",
        ),
        height=300,
        margin=dict(l=120, r=60, t=20, b=40),
        showlegend=False,
    )

    return fig
