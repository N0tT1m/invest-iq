"""Backtest Panel Component - equity curve, metrics, and trade history."""
import requests
import plotly.graph_objects as go
import dash_bootstrap_components as dbc
from dash import html, dcc

from components.config import API_BASE, get_headers, API_TIMEOUT


class BacktestPanelComponent:
    @staticmethod
    def fetch_backtest(symbol, days=365):
        """Fetch backtest results from the API."""
        try:
            response = requests.get(
                f"{API_BASE}/api/backtest/{symbol}",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"[backtest] Error fetching backtest for {symbol}: {e}")
            return None

    @staticmethod
    def create_panel(data, symbol):
        """Build the backtest results panel.

        Args:
            data: backtest result dict (or None)
            symbol: stock symbol
        """
        if data is None:
            return dbc.Alert(
                f"No backtest data available for {symbol}. The backend may still be processing.",
                color="warning",
            )

        children = []

        # Row 1: Metric cards
        children.append(BacktestPanelComponent._create_metrics_row(data))

        # Row 2: Equity curve
        equity_curve = data.get("equity_curve", [])
        initial_capital = data.get("initial_capital", 100000)
        if equity_curve:
            fig = BacktestPanelComponent._create_equity_curve(equity_curve, symbol, initial_capital)
            children.append(
                dcc.Graph(figure=fig, config={"displayModeBar": False}, className="mb-3")
            )

        # Row 3: Trade history table
        trades = data.get("trades", [])
        if trades:
            children.append(html.H6("Trade History", className="mb-2 mt-2"))
            children.append(BacktestPanelComponent._create_trade_table(trades))

        # Row 4: Footnotes
        total_comm = data.get("total_commission_paid", 0) or 0
        total_slip = data.get("total_slippage_cost", 0) or 0
        footnote_parts = []
        if total_comm > 0 or total_slip > 0:
            footnote_parts.append(f"Costs: ${total_comm:,.2f} commission + ${total_slip:,.2f} slippage")
        footnote_parts.append("Point-in-time signals (no look-ahead bias)")
        children.append(
            html.P(
                " | ".join(footnote_parts),
                className="text-muted small mt-2 mb-0",
            )
        )

        return html.Div(children)

    @staticmethod
    def _create_metrics_row(data):
        """Create metric summary cards (primary + extended)."""
        total_return = data.get("total_return", 0)
        total_return_pct = data.get("total_return_percent", 0)
        win_rate = data.get("win_rate", 0)
        total_trades = data.get("total_trades", 0)
        winning_trades = data.get("winning_trades", 0)
        sharpe = data.get("sharpe_ratio", 0)
        max_dd = data.get("max_drawdown", 0)

        ret_color = "text-success" if total_return >= 0 else "text-danger"
        ret_sign = "+" if total_return >= 0 else ""
        dd_color = "text-danger" if max_dd < -10 else "text-warning" if max_dd < -5 else "text-success"

        primary_cards = [
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Total Return", className="text-muted d-block"),
                html.H5(
                    f"{ret_sign}${total_return:,.2f} ({ret_sign}{total_return_pct:.1f}%)",
                    className=f"{ret_color} mb-0",
                ),
            ]), className="text-center"), md=3),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Win Rate", className="text-muted d-block"),
                html.H5(
                    f"{win_rate:.1f}%",
                    className="mb-0",
                ),
                html.Small(f"{winning_trades}/{total_trades} trades", className="text-muted"),
            ]), className="text-center"), md=3),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Sharpe Ratio", className="text-muted d-block"),
                html.H5(f"{sharpe:.2f}", className="mb-0"),
            ]), className="text-center"), md=3),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Max Drawdown", className="text-muted d-block"),
                html.H5(f"{max_dd:.1f}%", className=f"{dd_color} mb-0"),
            ]), className="text-center"), md=3),
        ]

        rows = [dbc.Row(primary_cards, className="mb-2")]

        # Extended risk metrics (if available)
        sortino = data.get("sortino_ratio")
        calmar = data.get("calmar_ratio")
        max_con_wins = data.get("max_consecutive_wins")
        max_con_losses = data.get("max_consecutive_losses")
        exposure = data.get("exposure_time_percent")
        recovery = data.get("recovery_factor")

        if any(v is not None for v in [sortino, calmar, max_con_wins, exposure]):
            extended_cards = [
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Sortino Ratio", className="text-muted d-block"),
                    html.H6(f"{sortino:.2f}" if sortino is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Calmar Ratio", className="text-muted d-block"),
                    html.H6(f"{calmar:.2f}" if calmar is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Recovery Factor", className="text-muted d-block"),
                    html.H6(f"{recovery:.2f}" if recovery is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Consec. Wins", className="text-muted d-block"),
                    html.H6(f"{max_con_wins}" if max_con_wins is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Consec. Losses", className="text-muted d-block"),
                    html.H6(f"{max_con_losses}" if max_con_losses is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
                dbc.Col(dbc.Card(dbc.CardBody([
                    html.Small("Exposure Time", className="text-muted d-block"),
                    html.H6(f"{exposure:.1f}%" if exposure is not None else "N/A", className="mb-0"),
                ]), className="text-center"), md=2),
            ]
            rows.append(dbc.Row(extended_cards, className="mb-3"))

        return html.Div(rows)

    @staticmethod
    def _create_equity_curve(equity_curve, symbol, initial_capital=100000):
        """Create equity curve line chart."""
        timestamps = [pt.get("timestamp", "") for pt in equity_curve]
        equities = [pt.get("equity", initial_capital) for pt in equity_curve]

        fig = go.Figure()
        fig.add_trace(go.Scatter(
            x=timestamps,
            y=equities,
            mode="lines",
            name="Equity",
            line=dict(color="#00ccff", width=2),
            fill="tozeroy",
            fillcolor="rgba(0, 204, 255, 0.08)",
        ))

        # Initial capital reference line
        fig.add_hline(
            y=initial_capital,
            line_dash="dash",
            line_color="rgba(255, 255, 255, 0.3)",
            annotation_text=f"Initial: ${initial_capital:,.0f}",
            annotation_position="right",
            annotation_font=dict(size=10, color="rgba(255,255,255,0.5)"),
        )

        fig.update_layout(
            template="plotly_dark",
            height=350,
            margin=dict(l=60, r=20, t=30, b=40),
            xaxis_title="Date",
            yaxis_title="Equity ($)",
            yaxis=dict(tickformat="$,.0f"),
            hovermode="x unified",
            showlegend=False,
        )
        return fig

    @staticmethod
    def _create_trade_table(trades, limit=20):
        """Create trade history table."""
        rows = []
        for trade in trades[:limit]:
            entry_date = trade.get("entry_date", "")[:10]
            exit_date = trade.get("exit_date", "")[:10]
            signal = trade.get("signal", "?")
            entry_price = trade.get("entry_price", 0)
            exit_price = trade.get("exit_price", 0)
            pnl = trade.get("profit_loss", 0)
            pnl_pct = trade.get("profit_loss_percent", 0)
            days = trade.get("holding_period_days", 0)
            reason = trade.get("exit_reason", "")

            pnl_color = "text-success" if pnl >= 0 else "text-danger"
            pnl_sign = "+" if pnl >= 0 else ""
            signal_color = "text-success" if signal.lower() in ("buy", "strongbuy", "weakbuy") else "text-danger"

            rows.append(html.Tr([
                html.Td(entry_date, className="small"),
                html.Td(exit_date, className="small"),
                html.Td(signal, className=f"{signal_color} fw-bold small"),
                html.Td(f"${entry_price:.2f}", className="small"),
                html.Td(f"${exit_price:.2f}", className="small"),
                html.Td(f"{pnl_sign}${pnl:,.2f}", className=f"{pnl_color} small"),
                html.Td(f"{pnl_sign}{pnl_pct:.1f}%", className=f"{pnl_color} small"),
                html.Td(str(days), className="small"),
                html.Td(reason, className="text-muted small"),
            ]))

        return dbc.Table(
            [html.Thead(html.Tr([
                html.Th("Entry"), html.Th("Exit"), html.Th("Signal"),
                html.Th("Entry$"), html.Th("Exit$"),
                html.Th("P&L"), html.Th("P&L%"), html.Th("Days"), html.Th("Reason"),
            ]))] + [html.Tbody(rows)],
            bordered=True, hover=True, responsive=True,
            className="table-dark table-sm",
        )
