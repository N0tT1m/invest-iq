"""Agent Trade Approval Panel — review and approve/reject trades proposed by the trading agent."""
import requests
import dash_bootstrap_components as dbc
from dash import html

from components.config import API_BASE, get_headers, API_TIMEOUT


class AgentTradesComponent:
    @staticmethod
    def fetch_pending_trades():
        """Fetch all pending trades from the agent queue."""
        try:
            response = requests.get(
                f"{API_BASE}/api/agent/trades",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data", []) if data.get("success") else []
        except Exception as e:
            print(f"Error fetching agent trades: {e}")
            return []

    @staticmethod
    def review_trade(trade_id, action):
        """Approve or reject a pending trade."""
        try:
            response = requests.post(
                f"{API_BASE}/api/agent/trades/{trade_id}/review",
                json={"action": action},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            return response.json()
        except Exception as e:
            return {"success": False, "error": str(e)}

    @staticmethod
    def create_panel(trades):
        """Build the agent trade approval panel."""
        pending = [t for t in trades if t.get("status") == "pending"]
        recent = [t for t in trades if t.get("status") != "pending"][:10]

        if not pending and not recent:
            return html.Div([
                html.P(
                    "No agent trade proposals yet. The trading agent will queue trades here for your approval.",
                    className="text-muted text-center py-3",
                ),
            ])

        children = []

        # Pending trades needing approval
        if pending:
            children.append(html.H6(f"Pending Approval ({len(pending)})", className="text-warning mb-2"))
            for trade in pending:
                children.append(_trade_card(trade, show_actions=True))
        else:
            children.append(html.P("No trades awaiting approval.", className="text-muted small mb-2"))

        # Recent history
        if recent:
            children.append(html.Hr())
            children.append(html.H6("Recent Decisions", className="text-muted mb-2"))
            for trade in recent[:5]:
                children.append(_trade_card(trade, show_actions=False))

        return html.Div(children)


def _trade_card(trade, show_actions=False):
    """Render a single trade proposal card."""
    trade_id = trade.get("id", 0)
    symbol = trade.get("symbol", "???")
    action = trade.get("action", "?")
    shares = trade.get("shares", 0)
    confidence = trade.get("confidence", 0) * 100
    reason = trade.get("reason", "")
    signal_type = trade.get("signal_type", "")
    status = trade.get("status", "pending")
    proposed_at = trade.get("proposed_at", "")[:16]

    action_color = "success" if action == "buy" else "danger"
    status_color = {
        "pending": "warning",
        "executed": "success",
        "rejected": "secondary",
        "expired": "dark",
    }.get(status, "info")

    body_children = [
        dbc.Row([
            dbc.Col([
                html.Span(f"{action.upper()} ", className=f"text-{action_color} fw-bold"),
                html.Span(f"{shares:g} shares of "),
                html.Span(symbol, className="fw-bold"),
            ], md=5),
            dbc.Col([
                html.Small(f"Signal: {signal_type}", className="text-muted d-block"),
                html.Small(f"Confidence: {confidence:.0f}%", className="text-muted"),
            ], md=3),
            dbc.Col([
                dbc.Badge(status.upper(), color=status_color, className="me-1"),
                html.Small(proposed_at, className="text-muted ms-1"),
            ], md=4, className="text-end"),
        ]),
    ]

    if reason:
        body_children.append(
            html.P(reason, className="small text-muted mt-1 mb-0 fst-italic")
        )

    if show_actions:
        body_children.append(
            dbc.Row([
                dbc.Col([
                    dbc.Button(
                        "Approve",
                        id={"type": "agent-approve-btn", "index": trade_id},
                        color="success",
                        size="sm",
                        className="w-100",
                    ),
                ], md=6),
                dbc.Col([
                    dbc.Button(
                        "Reject",
                        id={"type": "agent-reject-btn", "index": trade_id},
                        color="outline-danger",
                        size="sm",
                        className="w-100",
                    ),
                ], md=6),
            ], className="mt-2")
        )

    return dbc.Card(
        dbc.CardBody(body_children, className="py-2 px-3"),
        className="mb-2",
    )
