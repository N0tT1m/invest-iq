"""Stock Screener Component — multi-criteria filter builder + results table."""
import requests
import dash_bootstrap_components as dbc
from dash import html, dcc, dash_table

from components.config import API_BASE, get_headers, API_TIMEOUT


class StockScreenerComponent:

    @staticmethod
    def fetch_presets():
        try:
            resp = requests.get(
                f"{API_BASE}/api/screener/presets",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data", []) if data.get("success") else []
        except Exception:
            return []

    @staticmethod
    def run_scan(filters, universe="popular", mode="quick", sort_by="score", limit=30):
        try:
            payload = {
                "filters": filters,
                "universe": universe,
                "mode": mode,
                "sort_by": sort_by,
                "limit": limit,
            }
            resp = requests.post(
                f"{API_BASE}/api/screener/scan",
                headers=get_headers(),
                json=payload,
                timeout=API_TIMEOUT,
            )
            data = resp.json()
            return data.get("data") if data.get("success") else None
        except Exception as e:
            print(f"Screener error: {e}")
            return None

    @staticmethod
    def create_panel():
        presets = StockScreenerComponent.fetch_presets()
        preset_options = [{"label": p["name"], "value": p["id"]} for p in presets]
        preset_options.insert(0, {"label": "Custom Filters", "value": "custom"})

        return dbc.Card([
            dbc.CardHeader([
                html.H5("Stock Screener", className="mb-0 d-inline"),
                dbc.Badge("SCAN", color="info", className="ms-2"),
            ]),
            dbc.CardBody([
                # Controls row
                dbc.Row([
                    dbc.Col([
                        dbc.Label("Preset", size="sm"),
                        dcc.Dropdown(
                            id="screener-preset",
                            options=preset_options,
                            value="custom",
                            clearable=False,
                            style={"fontSize": "13px"},
                        ),
                    ], md=3),
                    dbc.Col([
                        dbc.Label("Universe", size="sm"),
                        dcc.Dropdown(
                            id="screener-universe",
                            options=[
                                {"label": "Popular (45)", "value": "popular"},
                                {"label": "Tech", "value": "tech"},
                                {"label": "Blue Chip", "value": "bluechip"},
                                {"label": "Dividend", "value": "dividend"},
                            ],
                            value="popular",
                            clearable=False,
                            style={"fontSize": "13px"},
                        ),
                    ], md=2),
                    dbc.Col([
                        dbc.Label("Mode", size="sm"),
                        dcc.Dropdown(
                            id="screener-mode",
                            options=[
                                {"label": "Quick (bars only)", "value": "quick"},
                                {"label": "Full Analysis", "value": "full"},
                            ],
                            value="quick",
                            clearable=False,
                            style={"fontSize": "13px"},
                        ),
                    ], md=2),
                    dbc.Col([
                        dbc.Label("Sort By", size="sm"),
                        dcc.Dropdown(
                            id="screener-sort",
                            options=[
                                {"label": "Score", "value": "score"},
                                {"label": "Momentum", "value": "momentum"},
                                {"label": "RSI", "value": "rsi"},
                                {"label": "Volume", "value": "volume"},
                                {"label": "Price", "value": "price"},
                            ],
                            value="score",
                            clearable=False,
                            style={"fontSize": "13px"},
                        ),
                    ], md=2),
                    dbc.Col([
                        dbc.Label("\u00a0", size="sm"),
                        html.Div(
                            dbc.Button(
                                "Scan",
                                id="screener-scan-btn",
                                color="primary",
                                size="sm",
                                className="w-100",
                            ),
                        ),
                    ], md=1),
                ], className="mb-3"),

                # Filter builder (accordion)
                dbc.Accordion([
                    dbc.AccordionItem([
                        StockScreenerComponent._filter_row("screener-f1", "rsi", "lt", "30"),
                        StockScreenerComponent._filter_row("screener-f2", "momentum_5d", "gt", "0"),
                        StockScreenerComponent._filter_row("screener-f3", "confidence", "gt", "0.5"),
                    ], title="Filters (up to 3)", item_id="filters"),
                ], start_collapsed=True, id="screener-filters-accordion", className="mb-3"),

                # Results
                dcc.Loading(
                    html.Div(id="screener-results", children=[
                        html.P("Click 'Scan' to search for stocks matching your criteria.",
                               className="text-muted text-center py-4")
                    ]),
                    type="circle",
                ),
            ]),
        ])

    @staticmethod
    def _filter_row(row_id, default_field, default_op, default_val):
        fields = [
            {"label": "RSI", "value": "rsi"},
            {"label": "Price", "value": "price"},
            {"label": "SMA-20 %", "value": "sma_20_pct"},
            {"label": "SMA-50 %", "value": "sma_50_pct"},
            {"label": "Volume Ratio", "value": "volume_ratio"},
            {"label": "5d Momentum %", "value": "momentum_5d"},
            {"label": "Signal Score", "value": "signal_score"},
            {"label": "Confidence", "value": "confidence"},
        ]
        ops = [
            {"label": ">", "value": "gt"},
            {"label": "<", "value": "lt"},
            {"label": ">=", "value": "gte"},
            {"label": "<=", "value": "lte"},
            {"label": "=", "value": "eq"},
        ]
        return dbc.Row([
            dbc.Col(dcc.Dropdown(
                id=f"{row_id}-field",
                options=fields,
                value=default_field,
                clearable=True,
                placeholder="Field",
                style={"fontSize": "12px"},
            ), md=4),
            dbc.Col(dcc.Dropdown(
                id=f"{row_id}-op",
                options=ops,
                value=default_op,
                clearable=False,
                style={"fontSize": "12px"},
            ), md=3),
            dbc.Col(dbc.Input(
                id=f"{row_id}-val",
                type="number",
                value=default_val,
                size="sm",
                style={"fontSize": "12px"},
            ), md=5),
        ], className="mb-2")

    @staticmethod
    def build_filters_from_inputs(fields, ops, vals):
        """Build filter list from the 3 filter row inputs."""
        filters = []
        for field, op, val in zip(fields, ops, vals):
            if field and val is not None and val != "":
                try:
                    v = float(val)
                except (ValueError, TypeError):
                    continue
                filters.append({"field": field, "operator": op, "value": v})
        return filters

    @staticmethod
    def create_results(scan_data):
        if not scan_data:
            return html.P("No results or scan failed.", className="text-muted text-center py-3")

        results = scan_data.get("results", [])
        total_scanned = scan_data.get("total_scanned", 0)
        total_matched = scan_data.get("total_matched", 0)
        mode = scan_data.get("mode", "quick")

        if not results:
            return html.Div([
                html.P(f"Scanned {total_scanned} stocks - no matches found.",
                       className="text-warning text-center py-3"),
                html.P("Try adjusting your filters.", className="text-muted text-center"),
            ])

        # Summary badge row
        summary = dbc.Row([
            dbc.Col(dbc.Badge(f"{total_matched} matched / {total_scanned} scanned",
                              color="info", className="me-2")),
            dbc.Col(dbc.Badge(f"Mode: {mode}", color="secondary")),
        ], className="mb-2")

        # Build table data
        rows = []
        for r in results:
            row = {
                "Symbol": r.get("symbol", ""),
                "Price": f"${r.get('price', 0):.2f}",
                "RSI": f"{r['rsi']:.1f}" if r.get("rsi") is not None else "-",
                "SMA20%": f"{r['sma_20_pct']:+.1f}%" if r.get("sma_20_pct") is not None else "-",
                "Mom5d": f"{r['momentum_5d']:+.1f}%" if r.get("momentum_5d") is not None else "-",
                "VolRatio": f"{r['volume_ratio']:.2f}x" if r.get("volume_ratio") is not None else "-",
                "Score": f"{r.get('signal_score', 0):.2f}",
                "Conf": f"{r.get('confidence', 0):.0%}",
                "Tags": ", ".join(r.get("tags", [])) if r.get("tags") else "",
            }
            if mode == "full":
                row["Signal"] = r.get("overall_signal", "-")
                row["Rec"] = (r.get("recommendation", "-") or "-")[:30]
            rows.append(row)

        columns = ["Symbol", "Price", "RSI", "SMA20%", "Mom5d", "VolRatio", "Score", "Conf", "Tags"]
        if mode == "full":
            columns += ["Signal", "Rec"]

        table = dash_table.DataTable(
            data=rows,
            columns=[{"name": c, "id": c} for c in columns],
            style_table={"overflowX": "auto"},
            style_cell={
                "textAlign": "center",
                "padding": "6px 10px",
                "fontSize": "13px",
                "fontFamily": "monospace",
            },
            style_header={
                "backgroundColor": "#1a1a2e",
                "color": "#e0e0e0",
                "fontWeight": "bold",
                "fontSize": "12px",
            },
            style_data={
                "backgroundColor": "#16213e",
                "color": "#e0e0e0",
            },
            style_data_conditional=[
                {
                    "if": {"row_index": "odd"},
                    "backgroundColor": "#0f3460",
                },
            ],
            page_size=20,
            sort_action="native",
        )

        return html.Div([summary, table])
