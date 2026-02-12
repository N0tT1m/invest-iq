"""Portfolio Analytics Component — risk metrics, performance, benchmark, rebalancing."""
import requests
from dash import html, dcc
import plotly.graph_objects as go
from .config import API_BASE, API_TIMEOUT, get_headers


class PortfolioAnalyticsComponent:

    @staticmethod
    def fetch_risk_metrics(days=90):
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/risk-metrics",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return None

    @staticmethod
    def fetch_performance(days=365):
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/performance-analytics",
                params={"days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return None

    @staticmethod
    def fetch_benchmark(symbol="SPY", days=365):
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/benchmark",
                params={"symbol": symbol, "days": days},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return None

    @staticmethod
    def fetch_drift():
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/drift",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return None

    @staticmethod
    def fetch_allocations():
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/allocations",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return []

    @staticmethod
    def fetch_rebalance():
        try:
            resp = requests.get(
                f"{API_BASE}/api/portfolio/rebalance",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            if data.get("success"):
                return data["data"]
        except Exception:
            pass
        return None

    # ---- Panel builders ----

    @staticmethod
    def create_risk_panel(metrics):
        if not metrics:
            return html.Div(
                "No risk data available. Save portfolio snapshots first.",
                style={"color": "#999", "textAlign": "center", "padding": "40px"},
            )

        cards = []
        card_data = [
            ("Sharpe", metrics.get("sharpe_ratio"), "{:.2f}"),
            ("Sortino", metrics.get("sortino_ratio"), "{:.2f}"),
            ("Max DD", metrics.get("max_drawdown_percent"), "{:.1f}%"),
            ("Current DD", metrics.get("current_drawdown_percent"), "{:.1f}%"),
            ("VaR 95%", metrics.get("var_95"), "{:.2%}"),
            ("CVaR 95%", metrics.get("cvar_95"), "{:.2%}"),
            ("HHI", metrics.get("herfindahl_index"), "{:.3f}"),
            ("Vol 20d", metrics.get("rolling_volatility_20d"), "{:.1%}"),
        ]
        for label, value, fmt in card_data:
            display = fmt.format(value) if value is not None else "N/A"
            cards.append(
                html.Div(
                    [
                        html.Div(label, style={"fontSize": "11px", "color": "#aaa"}),
                        html.Div(display, style={"fontSize": "18px", "fontWeight": "bold"}),
                    ],
                    style={
                        "background": "#1e1e2f",
                        "borderRadius": "8px",
                        "padding": "12px",
                        "textAlign": "center",
                        "minWidth": "80px",
                    },
                )
            )

        # Concentration donut
        top = metrics.get("top_holdings", [])
        donut = go.Figure()
        if top:
            donut.add_trace(
                go.Pie(
                    labels=[h["symbol"] for h in top],
                    values=[h["weight_percent"] for h in top],
                    hole=0.5,
                    textinfo="label+percent",
                    marker=dict(colors=["#636EFA", "#EF553B", "#00CC96", "#AB63FA", "#FFA15A"]),
                )
            )
            donut.update_layout(
                title="Concentration",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
                font=dict(color="white"),
                height=250,
                margin=dict(t=30, b=10, l=10, r=10),
                showlegend=False,
            )

        return html.Div(
            [
                html.Div(cards, style={"display": "flex", "gap": "8px", "flexWrap": "wrap", "marginBottom": "12px"}),
                dcc.Graph(figure=donut, config={"displayModeBar": False}) if top else html.Div(),
            ]
        )

    @staticmethod
    def create_performance_panel(analytics):
        if not analytics:
            return html.Div(
                "No performance data available.",
                style={"color": "#999", "textAlign": "center", "padding": "40px"},
            )

        # Summary cards
        cards_data = [
            ("Total Return", analytics.get("total_return_percent"), "{:.1f}%"),
            ("TWR", analytics.get("twr_percent"), "{:.1f}%"),
            ("30d Return", analytics.get("rolling_30d_return"), "{:.1f}%"),
            ("90d Return", analytics.get("rolling_90d_return"), "{:.1f}%"),
            ("YTD", analytics.get("ytd_return"), "{:.1f}%"),
            ("1Y Return", analytics.get("rolling_1y_return"), "{:.1f}%"),
        ]
        cards = []
        for label, value, fmt in cards_data:
            display = fmt.format(value) if value is not None else "N/A"
            color = "#00CC96" if value and value > 0 else "#EF553B" if value and value < 0 else "#aaa"
            cards.append(
                html.Div(
                    [
                        html.Div(label, style={"fontSize": "11px", "color": "#aaa"}),
                        html.Div(display, style={"fontSize": "18px", "fontWeight": "bold", "color": color}),
                    ],
                    style={
                        "background": "#1e1e2f",
                        "borderRadius": "8px",
                        "padding": "12px",
                        "textAlign": "center",
                        "minWidth": "80px",
                    },
                )
            )

        # Monthly returns heatmap
        monthly = analytics.get("monthly_returns", [])
        heatmap = go.Figure()
        if monthly:
            months_map = {}
            for m in monthly:
                months_map[(m["year"], m["month"])] = m["return_percent"]
            years = sorted(set(m["year"] for m in monthly))
            z = []
            for year in years:
                row = [months_map.get((year, mo), None) for mo in range(1, 13)]
                z.append(row)
            heatmap.add_trace(
                go.Heatmap(
                    z=z,
                    x=["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
                    y=[str(y) for y in years],
                    colorscale=[[0, "#EF553B"], [0.5, "#1e1e2f"], [1, "#00CC96"]],
                    zmid=0,
                    text=[[f"{v:.1f}%" if v is not None else "" for v in row] for row in z],
                    texttemplate="%{text}",
                    hovertemplate="Year: %{y}<br>Month: %{x}<br>Return: %{z:.1f}%<extra></extra>",
                )
            )
            heatmap.update_layout(
                title="Monthly Returns",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
                font=dict(color="white"),
                height=200,
                margin=dict(t=30, b=10, l=40, r=10),
            )

        # Attribution table
        attr = analytics.get("symbol_attribution", [])
        attr_rows = []
        for a in sorted(attr, key=lambda x: abs(x.get("contribution_percent", 0)), reverse=True)[:10]:
            color = "#00CC96" if a.get("contribution_percent", 0) >= 0 else "#EF553B"
            attr_rows.append(
                html.Tr(
                    [
                        html.Td(a["symbol"], style={"fontWeight": "bold"}),
                        html.Td(f'{a.get("weight_percent", 0):.1f}%'),
                        html.Td(f'{a.get("return_percent", 0):.1f}%', style={"color": color}),
                        html.Td(f'{a.get("contribution_percent", 0):.2f}%', style={"color": color}),
                    ],
                    style={"borderBottom": "1px solid #333"},
                )
            )

        attr_table = html.Table(
            [
                html.Thead(html.Tr([html.Th(h) for h in ["Symbol", "Weight", "Return", "Contrib."]])),
                html.Tbody(attr_rows),
            ],
            style={"width": "100%", "fontSize": "12px", "color": "white"},
        ) if attr_rows else html.Div()

        return html.Div(
            [
                html.Div(cards, style={"display": "flex", "gap": "8px", "flexWrap": "wrap", "marginBottom": "12px"}),
                dcc.Graph(figure=heatmap, config={"displayModeBar": False}) if monthly else html.Div(),
                attr_table,
            ]
        )

    @staticmethod
    def create_benchmark_panel(benchmark):
        if not benchmark:
            return html.Div(
                "No benchmark data available.",
                style={"color": "#999", "textAlign": "center", "padding": "40px"},
            )

        # Alpha/Beta/R2 cards
        cards_data = [
            ("Alpha", benchmark.get("alpha"), "{:.2%}"),
            ("Beta", benchmark.get("beta"), "{:.2f}"),
            ("R-Squared", benchmark.get("r_squared"), "{:.2f}"),
            ("Tracking Err", benchmark.get("tracking_error"), "{:.2%}"),
            ("Info Ratio", benchmark.get("information_ratio"), "{:.2f}"),
            ("Excess Return", benchmark.get("excess_return_percent"), "{:.1f}%"),
        ]
        cards = []
        for label, value, fmt in cards_data:
            display = fmt.format(value) if value is not None else "N/A"
            cards.append(
                html.Div(
                    [
                        html.Div(label, style={"fontSize": "11px", "color": "#aaa"}),
                        html.Div(display, style={"fontSize": "16px", "fontWeight": "bold"}),
                    ],
                    style={
                        "background": "#1e1e2f",
                        "borderRadius": "8px",
                        "padding": "10px",
                        "textAlign": "center",
                        "minWidth": "80px",
                    },
                )
            )

        # Indexed performance chart
        fig = go.Figure()
        pi = benchmark.get("portfolio_indexed", [])
        bi = benchmark.get("benchmark_indexed", [])
        if pi:
            fig.add_trace(
                go.Scatter(
                    x=[p["date"] for p in pi],
                    y=[p["value"] for p in pi],
                    name="Portfolio",
                    line=dict(color="#636EFA"),
                )
            )
        if bi:
            fig.add_trace(
                go.Scatter(
                    x=[p["date"] for p in bi],
                    y=[p["value"] for p in bi],
                    name=benchmark.get("benchmark_symbol", "SPY"),
                    line=dict(color="#FFA15A", dash="dot"),
                )
            )
        fig.update_layout(
            title="Portfolio vs Benchmark (Indexed to 100)",
            paper_bgcolor="rgba(0,0,0,0)",
            plot_bgcolor="rgba(0,0,0,0)",
            font=dict(color="white"),
            height=300,
            margin=dict(t=30, b=30, l=40, r=10),
            legend=dict(orientation="h", y=-0.15),
            yaxis=dict(gridcolor="#333"),
            xaxis=dict(gridcolor="#333"),
        )

        return html.Div(
            [
                html.Div(cards, style={"display": "flex", "gap": "8px", "flexWrap": "wrap", "marginBottom": "12px"}),
                dcc.Graph(figure=fig, config={"displayModeBar": False}),
            ]
        )

    @staticmethod
    def create_rebalance_panel(proposal, drift):
        children = []

        # Drift bar chart
        if drift:
            symbols = []
            drifts = []
            colors = []
            for d in drift:
                label = d.get("symbol") or d.get("sector") or "?"
                symbols.append(label)
                dv = d.get("drift_percent", 0)
                drifts.append(dv)
                colors.append("#EF553B" if d.get("needs_rebalance") else "#636EFA")

            fig = go.Figure()
            fig.add_trace(
                go.Bar(x=symbols, y=drifts, marker_color=colors, text=[f"{d:.1f}%" for d in drifts], textposition="outside")
            )
            fig.update_layout(
                title="Allocation Drift",
                paper_bgcolor="rgba(0,0,0,0)",
                plot_bgcolor="rgba(0,0,0,0)",
                font=dict(color="white"),
                height=250,
                margin=dict(t=30, b=30, l=40, r=10),
                yaxis=dict(title="Drift %", gridcolor="#333"),
            )
            children.append(dcc.Graph(figure=fig, config={"displayModeBar": False}))

        # Proposed trades table
        if proposal and proposal.get("trades"):
            rows = []
            for t in proposal["trades"]:
                color = "#00CC96" if t["action"] == "buy" else "#EF553B"
                rows.append(
                    html.Tr(
                        [
                            html.Td(t["symbol"], style={"fontWeight": "bold"}),
                            html.Td(t["action"].upper(), style={"color": color}),
                            html.Td(f'{t["shares"]:.2f}'),
                            html.Td(f'{t["current_weight_percent"]:.1f}%'),
                            html.Td(f'{t["target_weight_percent"]:.1f}%'),
                            html.Td(f'${t["estimated_value"]:.0f}'),
                        ],
                        style={"borderBottom": "1px solid #333"},
                    )
                )
            table = html.Table(
                [
                    html.Thead(html.Tr([html.Th(h) for h in ["Symbol", "Action", "Shares", "Current", "Target", "Value"]])),
                    html.Tbody(rows),
                ],
                style={"width": "100%", "fontSize": "12px", "color": "white", "marginTop": "12px"},
            )
            children.append(table)
            turnover = proposal.get("estimated_turnover_percent", 0)
            children.append(
                html.Div(
                    f"Estimated turnover: {turnover:.1f}%",
                    style={"color": "#aaa", "fontSize": "11px", "marginTop": "8px"},
                )
            )

        if not children:
            return html.Div(
                "Set target allocations to see rebalancing suggestions.",
                style={"color": "#999", "textAlign": "center", "padding": "40px"},
            )

        return html.Div(children)

    @staticmethod
    def create_layout():
        return html.Div(
            [
                dcc.Tabs(
                    id="analytics-tabs",
                    value="risk",
                    children=[
                        dcc.Tab(label="Risk Metrics", value="risk"),
                        dcc.Tab(label="Performance", value="performance"),
                        dcc.Tab(label="Benchmark", value="benchmark"),
                        dcc.Tab(label="Rebalance", value="rebalance"),
                    ],
                    colors={
                        "border": "#333",
                        "primary": "#636EFA",
                        "background": "#1a1a2e",
                    },
                ),
                html.Div(id="analytics-tab-content", style={"padding": "12px"}),
            ]
        )

    @staticmethod
    def _empty_results():
        return html.Div(
            "No analytics data available.",
            style={"color": "#999", "textAlign": "center", "padding": "40px"},
        )
