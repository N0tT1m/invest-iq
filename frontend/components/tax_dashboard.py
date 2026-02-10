"""
Tax-Loss Harvesting Dashboard Component

Displays tax-loss harvesting opportunities, wash sale monitoring,
and year-end tax summaries with multi-jurisdiction support.
"""

import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html
from typing import Dict, List, Optional
import requests

from components.config import API_BASE, get_headers, API_TIMEOUT


# Colors
COLORS = {
    "success": "#00cc88",
    "danger": "#ff6b6b",
    "warning": "#ffd93d",
    "info": "#4dabf7",
    "muted": "#888888",
}


class TaxDashboardComponent:
    """Component for displaying tax optimization features"""

    @staticmethod
    def fetch_harvest_opportunities(jurisdiction: str = "US") -> Optional[Dict]:
        """Fetch harvest opportunities from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/tax/harvest-opportunities",
                params={"jurisdiction": jurisdiction},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching harvest opportunities: {e}")
            return None

    @staticmethod
    def fetch_wash_sales(jurisdiction: str = "US") -> Optional[Dict]:
        """Fetch wash sale data from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/tax/wash-sales",
                params={"jurisdiction": jurisdiction},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching wash sales: {e}")
            return None

    @staticmethod
    def fetch_year_end_summary(year: int, jurisdiction: str = "US") -> Optional[Dict]:
        """Fetch year-end tax summary from API"""
        try:
            response = requests.get(
                f"{API_BASE}/api/tax/year-end-summary",
                params={"year": year, "jurisdiction": jurisdiction},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching year-end summary: {e}")
            return None

    @staticmethod
    def create_jurisdiction_selector(selected: str = "US") -> dbc.Select:
        """Create jurisdiction dropdown"""
        return dbc.Select(
            id="tax-jurisdiction-select",
            options=[
                {"label": "United States", "value": "US"},
                {"label": "United Kingdom", "value": "UK"},
                {"label": "Canada", "value": "CA"},
                {"label": "Australia", "value": "AU"},
                {"label": "Germany", "value": "DE"},
            ],
            value=selected,
            className="mb-3",
        )

    @staticmethod
    def create_summary_banner(summary: Dict) -> dbc.Alert:
        """Create summary banner showing total savings potential"""
        total_savings = summary.get("total_potential_savings", 0)
        total_losses = summary.get("total_harvestable_losses", 0)
        opportunity_count = summary.get("total_opportunities", 0)

        if total_savings <= 0:
            return dbc.Alert(
                [
                    "No tax-loss harvesting opportunities found. ",
                    html.Small(
                        "Opportunities appear when portfolio positions have unrealized losses.",
                        className="text-muted",
                    ),
                ],
                color="info",
                className="mb-3",
            )

        return dbc.Alert(
            [
                html.H4(
                    [
                        html.I(className="bi bi-piggy-bank me-2"),
                        f"${total_savings:,.0f} Potential Tax Savings",
                    ],
                    className="alert-heading mb-2",
                ),
                html.P(
                    [
                        f"Found {opportunity_count} opportunities with ",
                        html.Strong(f"${total_losses:,.0f}"),
                        " in harvestable losses.",
                    ],
                    className="mb-0",
                ),
            ],
            color="success",
            className="mb-3",
        )

    @staticmethod
    def create_opportunity_card(opp: Dict) -> dbc.Card:
        """Create a card for a harvest opportunity"""
        symbol = opp.get("symbol", "???")
        loss = opp.get("unrealized_loss", 0)
        savings = opp.get("estimated_tax_savings", 0)
        priority = opp.get("priority", "Low")
        loss_type = opp.get("loss_type", "Unknown")
        holding_days = opp.get("holding_days", 0)
        days_until_lt = opp.get("days_until_long_term")
        substitutes = opp.get("substitutes", [])

        priority_color = {
            "Urgent": "danger",
            "High": "warning",
            "Medium": "info",
            "Low": "secondary",
        }.get(priority, "secondary")

        # Create substitute badges
        sub_badges = []
        for sub in substitutes[:3]:  # Show max 3
            sub_badges.append(
                dbc.Badge(
                    sub.get("symbol", "?"),
                    color="primary" if sub.get("wash_sale_safe") else "secondary",
                    className="me-1",
                )
            )

        return dbc.Card(
            dbc.CardBody([
                html.Div([
                    html.H5(symbol, className="card-title mb-0"),
                    dbc.Badge(priority, color=priority_color, className="ms-2"),
                ], className="d-flex align-items-center mb-2"),
                html.Div([
                    html.Div([
                        html.Small("Loss", className="text-muted d-block"),
                        html.Span(
                            f"${loss:,.0f}",
                            style={"color": COLORS["danger"]},
                            className="fw-bold",
                        ),
                    ], className="me-4"),
                    html.Div([
                        html.Small("Tax Savings", className="text-muted d-block"),
                        html.Span(
                            f"${savings:,.0f}",
                            style={"color": COLORS["success"]},
                            className="fw-bold",
                        ),
                    ]),
                ], className="d-flex mb-3"),
                html.Div([
                    html.Small([
                        html.I(className="bi bi-calendar me-1"),
                        f"Held {holding_days} days ({loss_type})",
                    ], className="text-muted d-block mb-1"),
                    html.Small([
                        html.I(className="bi bi-clock me-1"),
                        f"{days_until_lt} days until long-term"
                        if days_until_lt else "Long-term eligible",
                    ], className="text-muted d-block"),
                ], className="mb-3"),
                html.Div([
                    html.Small("Substitutes:", className="text-muted me-2"),
                    *sub_badges,
                ]) if sub_badges else html.Div(),
                html.Hr(),
                html.P(
                    opp.get("reason", ""),
                    className="small text-muted mb-2",
                ),
                dbc.Button(
                    "Harvest",
                    id={"type": "harvest-btn", "index": opp.get("lot_id", "")},
                    color="success",
                    size="sm",
                    className="w-100",
                ),
            ]),
            className="mb-3 bg-dark border-secondary h-100",
        )

    @staticmethod
    def create_wash_sale_panel(wash_data: Dict) -> dbc.Card:
        """Create panel showing wash sale status"""
        windows = wash_data.get("windows", [])
        violations = wash_data.get("violations", [])
        summary = wash_data.get("summary", {})

        violation_count = summary.get("violation_count", 0)
        disallowed = summary.get("total_disallowed_loss", 0)

        header_color = "danger" if violation_count > 0 else "success"
        header_text = (
            f"{violation_count} Wash Sale Violations (${disallowed:,.0f} disallowed)"
            if violation_count > 0
            else "No Wash Sale Violations"
        )

        # Active windows
        active_windows = []
        for w in windows[:5]:  # Show max 5
            active_windows.append(
                html.Div([
                    html.Span(w.get("symbol", "?"), className="fw-bold me-2"),
                    html.Small([
                        f"Window: {w.get('window_start', '')} to {w.get('window_end', '')}",
                    ], className="text-muted"),
                    dbc.Badge(
                        f"{w.get('days_remaining', 0)} days left",
                        color="warning",
                        className="ms-2",
                    ) if not w.get("triggered") else dbc.Badge(
                        "Triggered",
                        color="danger",
                        className="ms-2",
                    ),
                ], className="mb-2")
            )

        return dbc.Card(
            dbc.CardBody([
                html.Div([
                    html.H5([
                        html.I(className="bi bi-exclamation-triangle me-2"),
                        "Wash Sale Monitor",
                    ], className="card-title mb-0"),
                    dbc.Badge(header_text, color=header_color, className="ms-2"),
                ], className="d-flex align-items-center mb-3"),
                html.Div(active_windows) if active_windows else html.P(
                    "No active wash sale windows.",
                    className="text-muted",
                ),
            ]),
            className="mb-3 bg-dark border-secondary",
        )


def create_savings_gauge(savings: float, max_savings: float = 5000) -> go.Figure:
    """
    Create a gauge showing potential tax savings

    Args:
        savings: Current savings potential
        max_savings: Maximum expected savings for gauge scale

    Returns:
        Plotly gauge figure
    """
    fig = go.Figure(
        go.Indicator(
            mode="gauge+number",
            value=savings,
            number={"prefix": "$", "valueformat": ",.0f"},
            title={"text": "Tax Savings Potential", "font": {"color": "#ffffff"}},
            gauge={
                "axis": {"range": [0, max_savings], "tickcolor": "#ffffff"},
                "bar": {"color": COLORS["success"]},
                "bgcolor": "rgba(0,0,0,0)",
                "borderwidth": 2,
                "bordercolor": "#444",
                "steps": [
                    {"range": [0, max_savings * 0.33], "color": "rgba(255,107,107,0.3)"},
                    {"range": [max_savings * 0.33, max_savings * 0.66], "color": "rgba(255,217,61,0.3)"},
                    {"range": [max_savings * 0.66, max_savings], "color": "rgba(0,204,136,0.3)"},
                ],
            },
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        font={"color": "#ffffff"},
        height=250,
        margin={"l": 40, "r": 40, "t": 60, "b": 20},
    )

    return fig


def create_year_end_chart(summary: Dict) -> go.Figure:
    """
    Create a bar chart showing gains vs losses

    Args:
        summary: Year-end summary data

    Returns:
        Plotly figure
    """
    categories = ["Short-Term", "Long-Term"]
    gains = [
        summary.get("short_term_gains", 0),
        summary.get("long_term_gains", 0),
    ]
    losses = [
        -summary.get("short_term_losses", 0),
        -summary.get("long_term_losses", 0),
    ]

    fig = go.Figure()

    fig.add_trace(
        go.Bar(
            name="Gains",
            x=categories,
            y=gains,
            marker_color=COLORS["success"],
            text=[f"${g:,.0f}" for g in gains],
            textposition="outside",
        )
    )

    fig.add_trace(
        go.Bar(
            name="Losses",
            x=categories,
            y=losses,
            marker_color=COLORS["danger"],
            text=[f"${abs(l):,.0f}" for l in losses],
            textposition="outside",
        )
    )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font={"color": "#ffffff"},
        title={
            "text": "Capital Gains & Losses",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        barmode="group",
        xaxis={"gridcolor": "rgba(128,128,128,0.2)"},
        yaxis={
            "title": "Amount ($)",
            "gridcolor": "rgba(128,128,128,0.2)",
            "zeroline": True,
            "zerolinecolor": "#ffffff",
        },
        height=300,
        margin={"l": 60, "r": 40, "t": 60, "b": 40},
        legend={"orientation": "h", "y": -0.15},
    )

    return fig


def create_tax_summary_card(summary: Dict) -> dbc.Card:
    """
    Create a card showing year-end tax summary

    Args:
        summary: Year-end summary data

    Returns:
        Dash Card component
    """
    net_total = summary.get("total_net", 0)
    estimated_tax = summary.get("estimated_tax", 0)
    carryforward = summary.get("loss_carryforward", 0)

    net_color = COLORS["success"] if net_total < 0 else COLORS["danger"]
    tax_text = (
        f"${abs(estimated_tax):,.0f} savings"
        if estimated_tax < 0
        else f"${estimated_tax:,.0f} owed"
    )

    return dbc.Card(
        dbc.CardBody([
            html.H5([
                html.I(className="bi bi-file-earmark-text me-2"),
                f"Tax Year {summary.get('tax_year', '')} Summary",
            ], className="card-title mb-3"),
            html.Div([
                html.Div([
                    html.Small("Net Capital Gain/Loss", className="text-muted d-block"),
                    html.H4(
                        f"${net_total:+,.0f}",
                        style={"color": net_color},
                    ),
                ], className="mb-3"),
                html.Div([
                    html.Small("Estimated Tax Impact", className="text-muted d-block"),
                    html.H5(tax_text),
                ], className="mb-3"),
                html.Div([
                    html.Small("Loss Carryforward", className="text-muted d-block"),
                    html.Span(f"${carryforward:,.0f}"),
                ]) if carryforward > 0 else html.Div(),
            ]),
            html.Hr(),
            html.Small([
                html.I(className="bi bi-info-circle me-1"),
                summary.get("jurisdiction", "US"),
                " tax rules applied. This is an estimate only.",
            ], className="text-muted"),
        ]),
        className="bg-dark border-secondary",
    )


def create_wash_sale_calendar(symbol: str, windows: List[Dict]) -> go.Figure:
    """
    Create a calendar-style visualization of wash sale windows

    Args:
        symbol: Stock symbol
        windows: List of wash sale windows

    Returns:
        Plotly figure
    """
    if not windows:
        fig = go.Figure()
        fig.add_annotation(
            x=0.5, y=0.5,
            text=f"No wash sale windows for {symbol}",
            showarrow=False,
            font={"size": 14, "color": "#888"},
        )
        fig.update_layout(
            paper_bgcolor="rgba(0,0,0,0)",
            plot_bgcolor="rgba(0,0,0,0)",
            height=200,
        )
        return fig

    # Create timeline
    fig = go.Figure()

    for i, window in enumerate(windows):
        start = window.get("window_start", "")
        end = window.get("window_end", "")
        triggered = window.get("triggered", False)

        color = COLORS["danger"] if triggered else COLORS["warning"]

        fig.add_trace(
            go.Scatter(
                x=[start, end],
                y=[i, i],
                mode="lines+markers",
                line={"color": color, "width": 8},
                marker={"size": 12},
                name=f"Window {i+1}",
                hovertemplate=f"<b>{symbol}</b><br>Window: {start} to {end}<br>{'Triggered' if triggered else 'Active'}<extra></extra>",
            )
        )

    fig.update_layout(
        paper_bgcolor="rgba(0,0,0,0)",
        plot_bgcolor="rgba(0,0,0,0)",
        font={"color": "#ffffff"},
        title={
            "text": f"Wash Sale Windows - {symbol}",
            "font": {"size": 14, "color": "#ffffff"},
            "x": 0.5,
        },
        xaxis={"title": "Date", "gridcolor": "rgba(128,128,128,0.2)"},
        yaxis={"visible": False},
        showlegend=False,
        height=200,
        margin={"l": 40, "r": 40, "t": 60, "b": 40},
    )

    return fig
