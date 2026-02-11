"""Backtest Panel Component - equity curve, metrics, trade history, benchmark, walk-forward, Monte Carlo."""
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
    def fetch_monte_carlo(backtest_id, simulations=1000):
        """Fetch Monte Carlo simulation for a backtest."""
        try:
            response = requests.get(
                f"{API_BASE}/api/backtest/results/{backtest_id}/monte-carlo",
                params={"simulations": simulations},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = response.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"[backtest] Error fetching Monte Carlo: {e}")
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

        # Row 1b: Benchmark metrics (if available)
        benchmark = data.get("benchmark")
        if benchmark:
            children.append(BacktestPanelComponent._create_benchmark_row(benchmark))

        # Row 2: Equity curve (with benchmark lines)
        equity_curve = data.get("equity_curve", [])
        initial_capital = data.get("initial_capital", 100000)
        if equity_curve:
            fig = BacktestPanelComponent._create_equity_curve(
                equity_curve, symbol, initial_capital, benchmark
            )
            children.append(
                dcc.Graph(figure=fig, config={"displayModeBar": False}, className="mb-3")
            )

        # Row 3: Trade history table
        trades = data.get("trades", [])
        if trades:
            children.append(html.H6("Trade History", className="mb-2 mt-2"))
            children.append(BacktestPanelComponent._create_trade_table(trades))

        # Row 4: Per-symbol breakdown (multi-symbol backtests)
        per_symbol = data.get("per_symbol_results")
        if per_symbol:
            children.append(html.H6("Per-Symbol Breakdown", className="mb-2 mt-2"))
            children.append(BacktestPanelComponent._create_per_symbol_table(per_symbol))

        # Row 5: Footnotes
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

    # --- Monte Carlo Panel ---

    @staticmethod
    def create_monte_carlo_panel(mc_data):
        """Build Monte Carlo simulation results panel."""
        if mc_data is None:
            return dbc.Alert("No Monte Carlo data available.", color="warning")

        children = []

        # Metric cards
        prob_profit = mc_data.get("probability_of_profit", 0)
        prob_ruin = mc_data.get("probability_of_ruin", 0)
        median_return = mc_data.get("median_return", 0)
        p5 = mc_data.get("percentile_5", 0)
        p95 = mc_data.get("percentile_95", 0)
        median_dd = mc_data.get("median_max_drawdown", 0)
        median_sharpe = mc_data.get("median_sharpe", 0)

        profit_color = "text-success" if prob_profit >= 50 else "text-danger"
        ruin_color = "text-danger" if prob_ruin > 5 else "text-success"

        mc_cards = [
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("P(Profit)", className="text-muted d-block"),
                html.H5(f"{prob_profit:.1f}%", className=f"{profit_color} mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("P(Ruin >50%)", className="text-muted d-block"),
                html.H5(f"{prob_ruin:.1f}%", className=f"{ruin_color} mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Median Return", className="text-muted d-block"),
                html.H5(f"{median_return:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("5th / 95th %ile", className="text-muted d-block"),
                html.H6(f"{p5:+.1f}% / {p95:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Median Max DD", className="text-muted d-block"),
                html.H6(f"{median_dd:.1f}%", className="text-warning mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Median Sharpe", className="text-muted d-block"),
                html.H6(f"{median_sharpe:.2f}", className="mb-0"),
            ]), className="text-center"), md=2),
        ]
        children.append(dbc.Row(mc_cards, className="mb-3"))

        # Return distribution histogram
        return_dist = mc_data.get("return_distribution", [])
        if return_dist:
            fig = go.Figure()
            fig.add_trace(go.Histogram(
                x=return_dist,
                nbinsx=50,
                marker_color="rgba(0, 204, 255, 0.7)",
                name="Return Distribution",
            ))
            fig.add_vline(x=0, line_dash="dash", line_color="rgba(255,255,255,0.5)")
            if median_return != 0:
                fig.add_vline(
                    x=median_return, line_dash="dot", line_color="#ffc107",
                    annotation_text=f"Median: {median_return:+.1f}%",
                    annotation_font=dict(size=10, color="#ffc107"),
                )
            fig.update_layout(
                template="plotly_dark",
                height=280,
                margin=dict(l=50, r=20, t=30, b=40),
                xaxis_title="Total Return (%)",
                yaxis_title="Frequency",
                showlegend=False,
                title_text=f"Monte Carlo Return Distribution ({mc_data.get('simulations', 0)} sims)",
                title_font_size=12,
            )
            children.append(dcc.Graph(figure=fig, config={"displayModeBar": False}, className="mb-2"))

        simulations = mc_data.get("simulations", 0)
        children.append(
            html.P(f"{simulations} simulations via trade-sequence reshuffling",
                   className="text-muted small")
        )

        return html.Div(children)

    # --- Walk-Forward Panel ---

    @staticmethod
    def create_walk_forward_panel(wf_data):
        """Build walk-forward validation results panel."""
        if wf_data is None:
            return dbc.Alert("No walk-forward data available.", color="warning")

        children = []

        # Summary metrics
        overfitting = wf_data.get("overfitting_ratio", 0)
        avg_is = wf_data.get("avg_in_sample_return", 0)
        avg_oos = wf_data.get("avg_out_of_sample_return", 0)
        oos_wr = wf_data.get("out_of_sample_win_rate", 0)
        oos_sharpe = wf_data.get("out_of_sample_sharpe")
        total_trades = wf_data.get("total_oos_trades", 0)

        of_color = "text-success" if 0.5 <= overfitting <= 2.0 else "text-danger"

        wf_cards = [
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Overfitting Ratio", className="text-muted d-block"),
                html.H5(f"{overfitting:.2f}", className=f"{of_color} mb-0"),
                html.Small("Near 1.0 = low overfitting", className="text-muted"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Avg In-Sample", className="text-muted d-block"),
                html.H6(f"{avg_is:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Avg Out-of-Sample", className="text-muted d-block"),
                html.H6(f"{avg_oos:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("OOS Win Rate", className="text-muted d-block"),
                html.H6(f"{oos_wr:.1f}%", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("OOS Sharpe", className="text-muted d-block"),
                html.H6(f"{oos_sharpe:.2f}" if oos_sharpe else "N/A", className="mb-0"),
            ]), className="text-center"), md=2),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("OOS Trades", className="text-muted d-block"),
                html.H6(str(total_trades), className="mb-0"),
            ]), className="text-center"), md=2),
        ]
        children.append(dbc.Row(wf_cards, className="mb-3"))

        # Fold table
        folds = wf_data.get("folds", [])
        if folds:
            rows = []
            for fold in folds:
                is_ret = fold.get("in_sample_return", 0)
                oos_ret = fold.get("out_of_sample_return", 0)
                oos_color = "text-success" if oos_ret >= 0 else "text-danger"
                rows.append(html.Tr([
                    html.Td(str(fold.get("fold_number", "")), className="small"),
                    html.Td(f'{fold.get("train_start", "")[:10]} - {fold.get("train_end", "")[:10]}', className="small"),
                    html.Td(f'{fold.get("test_start", "")[:10]} - {fold.get("test_end", "")[:10]}', className="small"),
                    html.Td(f"{is_ret:+.1f}%", className="small"),
                    html.Td(f"{oos_ret:+.1f}%", className=f"{oos_color} small"),
                    html.Td(str(fold.get("in_sample_trades", 0)), className="small"),
                    html.Td(str(fold.get("out_of_sample_trades", 0)), className="small"),
                ]))

            children.append(dbc.Table(
                [html.Thead(html.Tr([
                    html.Th("Fold"), html.Th("Train Period"), html.Th("Test Period"),
                    html.Th("IS Return"), html.Th("OOS Return"),
                    html.Th("IS Trades"), html.Th("OOS Trades"),
                ]))] + [html.Tbody(rows)],
                bordered=True, hover=True, responsive=True,
                className="table-dark table-sm",
            ))

        # OOS equity curve
        oos_curve = wf_data.get("combined_equity_curve", [])
        if oos_curve:
            timestamps = [p.get("timestamp", "") for p in oos_curve]
            equities = [p.get("equity", 0) for p in oos_curve]
            fig = go.Figure()
            fig.add_trace(go.Scatter(
                x=timestamps, y=equities, mode="lines",
                name="OOS Equity", line=dict(color="#ffc107", width=2),
            ))
            fig.update_layout(
                template="plotly_dark", height=250,
                margin=dict(l=50, r=20, t=30, b=40),
                xaxis_title="Date", yaxis_title="Equity ($)",
                yaxis=dict(tickformat="$,.0f"),
                showlegend=False,
                title_text="Combined Out-of-Sample Equity Curve",
                title_font_size=12,
            )
            children.append(dcc.Graph(figure=fig, config={"displayModeBar": False}))

        return html.Div(children)

    # --- Internal Helpers ---

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
    def _create_benchmark_row(benchmark):
        """Create benchmark comparison metric row."""
        bh_return = benchmark.get("buy_hold_return_percent", 0)
        alpha = benchmark.get("alpha", 0)
        spy_return = benchmark.get("spy_return_percent")
        spy_alpha = benchmark.get("spy_alpha")
        info_ratio = benchmark.get("information_ratio")

        alpha_color = "text-success" if alpha >= 0 else "text-danger"
        alpha_sign = "+" if alpha >= 0 else ""

        cards = [
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Buy & Hold Return", className="text-muted d-block"),
                html.H6(f"{bh_return:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=3),
            dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Alpha vs Buy & Hold", className="text-muted d-block"),
                html.H6(f"{alpha_sign}{alpha:.1f}%", className=f"{alpha_color} mb-0"),
            ]), className="text-center"), md=3),
        ]

        if spy_return is not None:
            cards.append(dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("SPY Return", className="text-muted d-block"),
                html.H6(f"{spy_return:+.1f}%", className="mb-0"),
            ]), className="text-center"), md=3))

        if spy_alpha is not None:
            spy_alpha_color = "text-success" if spy_alpha >= 0 else "text-danger"
            spy_alpha_sign = "+" if spy_alpha >= 0 else ""
            cards.append(dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Alpha vs SPY", className="text-muted d-block"),
                html.H6(f"{spy_alpha_sign}{spy_alpha:.1f}%", className=f"{spy_alpha_color} mb-0"),
            ]), className="text-center"), md=3))

        if info_ratio is not None and spy_return is None:
            cards.append(dbc.Col(dbc.Card(dbc.CardBody([
                html.Small("Information Ratio", className="text-muted d-block"),
                html.H6(f"{info_ratio:.2f}", className="mb-0"),
            ]), className="text-center"), md=3))

        return dbc.Row(cards, className="mb-2")

    @staticmethod
    def _create_equity_curve(equity_curve, symbol, initial_capital=100000, benchmark=None):
        """Create equity curve line chart with optional benchmark lines."""
        timestamps = [pt.get("timestamp", "") for pt in equity_curve]
        equities = [pt.get("equity", initial_capital) for pt in equity_curve]

        fig = go.Figure()
        fig.add_trace(go.Scatter(
            x=timestamps,
            y=equities,
            mode="lines",
            name="Strategy",
            line=dict(color="#00ccff", width=2),
            fill="tozeroy",
            fillcolor="rgba(0, 204, 255, 0.08)",
        ))

        # Buy-and-hold benchmark line
        if benchmark:
            bh_curve = benchmark.get("buy_hold_equity_curve", [])
            if bh_curve:
                bh_ts = [p.get("timestamp", "") for p in bh_curve]
                bh_eq = [p.get("equity", 0) for p in bh_curve]
                fig.add_trace(go.Scatter(
                    x=bh_ts, y=bh_eq, mode="lines",
                    name="Buy & Hold",
                    line=dict(color="rgba(255,255,255,0.4)", width=1, dash="dash"),
                ))

            # SPY benchmark line
            spy_curve = benchmark.get("spy_equity_curve", [])
            if spy_curve:
                spy_ts = [p.get("timestamp", "") for p in spy_curve]
                spy_eq = [p.get("equity", 0) for p in spy_curve]
                fig.add_trace(go.Scatter(
                    x=spy_ts, y=spy_eq, mode="lines",
                    name="SPY",
                    line=dict(color="#ffc107", width=1, dash="dot"),
                ))

        # Initial capital reference line
        fig.add_hline(
            y=initial_capital,
            line_dash="dash",
            line_color="rgba(255, 255, 255, 0.15)",
            annotation_text=f"Initial: ${initial_capital:,.0f}",
            annotation_position="right",
            annotation_font=dict(size=10, color="rgba(255,255,255,0.5)"),
        )

        show_legend = benchmark is not None
        fig.update_layout(
            template="plotly_dark",
            height=350,
            margin=dict(l=60, r=20, t=30, b=40),
            xaxis_title="Date",
            yaxis_title="Equity ($)",
            yaxis=dict(tickformat="$,.0f"),
            hovermode="x unified",
            showlegend=show_legend,
            legend=dict(orientation="h", yanchor="bottom", y=1.02, xanchor="right", x=1),
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

    @staticmethod
    def _create_per_symbol_table(per_symbol):
        """Create per-symbol breakdown table for multi-symbol backtests."""
        rows = []
        for sym in per_symbol:
            ret = sym.get("total_return", 0)
            ret_pct = sym.get("total_return_percent", 0)
            wr = sym.get("win_rate", 0)
            weight = sym.get("weight", 0) * 100
            ret_color = "text-success" if ret >= 0 else "text-danger"
            ret_sign = "+" if ret >= 0 else ""
            rows.append(html.Tr([
                html.Td(sym.get("symbol", ""), className="fw-bold small"),
                html.Td(f"{weight:.0f}%", className="small"),
                html.Td(str(sym.get("total_trades", 0)), className="small"),
                html.Td(f"{wr:.1f}%", className="small"),
                html.Td(f"{ret_sign}${ret:,.2f}", className=f"{ret_color} small"),
                html.Td(f"{ret_sign}{ret_pct:.1f}%", className=f"{ret_color} small"),
            ]))

        return dbc.Table(
            [html.Thead(html.Tr([
                html.Th("Symbol"), html.Th("Weight"), html.Th("Trades"),
                html.Th("Win Rate"), html.Th("P&L"), html.Th("Return%"),
            ]))] + [html.Tbody(rows)],
            bordered=True, hover=True, responsive=True,
            className="table-dark table-sm",
        )
