"""
Symbol Search Component

Full-page symbol search with:
- Real-time typeahead search via Polygon API
- Symbol detail cards with company info, price, market cap
- Quick-analyze button to jump to dashboard analysis
- Recent searches history
"""

import requests
import dash_bootstrap_components as dbc
from dash import html, dcc
from typing import Dict, List, Optional

from components.config import API_BASE, get_headers, API_TIMEOUT


class SymbolSearchComponent:
    """Component for searching and exploring symbols"""

    @staticmethod
    def fetch_search_results(query: str, limit: int = 20) -> Optional[List[Dict]]:
        """Search for symbols matching query text"""
        if not query or len(query.strip()) < 1:
            return None
        try:
            response = requests.get(
                f"{API_BASE}/api/symbols/search",
                params={"q": query.strip(), "limit": limit},
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data", [])
            return None
        except Exception as e:
            print(f"Error searching symbols: {e}")
            return None

    @staticmethod
    def fetch_symbol_detail(symbol: str) -> Optional[Dict]:
        """Get detailed info for a specific symbol"""
        try:
            response = requests.get(
                f"{API_BASE}/api/symbols/{symbol}",
                headers=get_headers(),
                timeout=API_TIMEOUT,
            )
            if response.status_code == 200:
                data = response.json()
                if data.get("success"):
                    return data.get("data")
            return None
        except Exception as e:
            print(f"Error fetching symbol detail: {e}")
            return None

    @staticmethod
    def create_search_result_card(result: Dict) -> dbc.Card:
        """Create a card for a single search result"""
        ticker = result.get("ticker", "???")
        name = result.get("name", "Unknown")
        exchange = result.get("exchange", "")
        symbol_type = result.get("symbol_type", "")
        currency = result.get("currency", "USD")

        exchange_display = exchange.replace("XNAS", "NASDAQ").replace("XNYS", "NYSE").replace("XASE", "AMEX")

        return dbc.Card(
            dbc.CardBody(
                dbc.Row([
                    dbc.Col([
                        html.Div([
                            html.Strong(
                                ticker,
                                className="fs-5 me-2",
                                style={"color": "#00cc88"},
                            ),
                            dbc.Badge(
                                exchange_display,
                                color="secondary",
                                className="me-1",
                            ),
                        ], className="d-flex align-items-center mb-1"),
                        html.P(
                            name,
                            className="mb-0 small text-muted",
                            style={"maxWidth": "400px", "overflow": "hidden", "textOverflow": "ellipsis", "whiteSpace": "nowrap"},
                        ),
                    ], md=8),
                    dbc.Col([
                        html.Div([
                            html.Small(f"{symbol_type} | {currency}", className="text-muted me-2"),
                            dbc.Button(
                                "Analyze",
                                id={"type": "search-analyze-btn", "index": ticker},
                                color="primary",
                                size="sm",
                                outline=True,
                            ),
                        ], className="d-flex align-items-center justify-content-end"),
                    ], md=4),
                ], align="center"),
            ),
            className="mb-2",
            style={"cursor": "pointer"},
        )

    @staticmethod
    def create_symbol_detail_card(detail: Dict) -> dbc.Card:
        """Create a detailed card for a symbol"""
        if not detail:
            return html.Div()

        ticker = detail.get("ticker", "???")
        name = detail.get("name", "Unknown")
        description = detail.get("description", "")
        market_cap = detail.get("market_cap")
        homepage = detail.get("homepage_url")
        sic_desc = detail.get("sic_description", "")
        employees = detail.get("total_employees")
        list_date = detail.get("list_date")
        current_price = detail.get("current_price")
        change_pct = detail.get("change_percent")

        # Format market cap
        if market_cap:
            if market_cap >= 1_000_000_000_000:
                mc_str = f"${market_cap / 1_000_000_000_000:.2f}T"
            elif market_cap >= 1_000_000_000:
                mc_str = f"${market_cap / 1_000_000_000:.2f}B"
            elif market_cap >= 1_000_000:
                mc_str = f"${market_cap / 1_000_000:.1f}M"
            else:
                mc_str = f"${market_cap:,.0f}"
        else:
            mc_str = "N/A"

        # Format price
        price_display = []
        if current_price is not None:
            price_display.append(
                html.Span(f"${current_price:.2f}", className="fs-4 fw-bold me-2")
            )
            if change_pct is not None:
                color = "text-success" if change_pct >= 0 else "text-danger"
                arrow = "+" if change_pct >= 0 else ""
                price_display.append(
                    html.Span(f"{arrow}{change_pct:.2f}%", className=f"{color} fs-5")
                )

        # Build info rows
        info_items = []
        if mc_str != "N/A":
            info_items.append(html.Span([html.Strong("Market Cap: "), mc_str], className="me-4"))
        if sic_desc:
            info_items.append(html.Span([html.Strong("Sector: "), sic_desc], className="me-4"))
        if employees:
            info_items.append(html.Span([html.Strong("Employees: "), f"{employees:,}"], className="me-4"))
        if list_date:
            info_items.append(html.Span([html.Strong("Listed: "), list_date], className="me-4"))

        return dbc.Card([
            dbc.CardHeader(
                html.Div([
                    html.Div([
                        html.H4(ticker, className="mb-0 me-3", style={"color": "#00cc88"}),
                        html.H5(name, className="mb-0 text-muted"),
                    ], className="d-flex align-items-center"),
                    html.Div(price_display) if price_display else None,
                ], className="d-flex justify-content-between align-items-center")
            ),
            dbc.CardBody([
                # Info row
                html.Div(info_items, className="mb-3 small") if info_items else None,
                # Description
                html.P(
                    description[:500] + "..." if description and len(description) > 500 else description,
                    className="small text-muted mb-3",
                ) if description else None,
                # Actions
                html.Div([
                    dbc.Button(
                        "Full Analysis",
                        id="search-detail-analyze-btn",
                        color="primary",
                        className="me-2",
                    ),
                    dbc.Button(
                        "Run Backtest",
                        id="search-detail-backtest-btn",
                        color="success",
                        outline=True,
                        className="me-2",
                    ),
                    html.A(
                        dbc.Button("Website", color="link", size="sm"),
                        href=homepage,
                        target="_blank",
                    ) if homepage else None,
                ]),
            ]),
        ], className="mb-4")

    @staticmethod
    def create_search_page_layout():
        """Create the full search page layout"""
        return html.Div([
            # Search header
            dbc.Row([
                dbc.Col([
                    html.H2("Symbol Search", className="mb-3"),
                    html.P("Search for any stock by ticker symbol or company name",
                           className="text-muted mb-4"),
                ])
            ]),
            # Search input
            dbc.Row([
                dbc.Col([
                    dbc.InputGroup([
                        dbc.InputGroupText(html.I(className="bi bi-search") if False else "Search"),
                        dbc.Input(
                            id="symbol-search-input",
                            type="text",
                            placeholder="Search by symbol or company name (e.g., AAPL, Apple, Tesla)...",
                            debounce=True,
                            className="form-control-lg",
                        ),
                    ], size="lg"),
                ], md=8),
                dbc.Col([
                    dbc.Button(
                        "Search",
                        id="symbol-search-btn",
                        color="primary",
                        size="lg",
                        className="w-100",
                        n_clicks=0,
                    ),
                ], md=2),
                dbc.Col([
                    dbc.Button(
                        "Clear",
                        id="symbol-search-clear-btn",
                        color="secondary",
                        size="lg",
                        outline=True,
                        className="w-100",
                        n_clicks=0,
                    ),
                ], md=2),
            ], className="mb-4"),
            # Detail section (shown when a symbol is selected)
            html.Div(id="symbol-detail-section"),
            # Results section
            dcc.Loading(
                html.Div(id="symbol-search-results"),
                type="circle",
            ),
            # Popular symbols section (shown when no search)
            html.Div(id="popular-symbols-section", children=[
                html.Hr(className="my-4"),
                html.H5("Popular Symbols", className="mb-3"),
                dbc.Row([
                    dbc.Col([
                        _popular_category("Tech Giants", ["AAPL", "MSFT", "GOOGL", "AMZN", "META", "NVDA"]),
                    ], md=4),
                    dbc.Col([
                        _popular_category("Finance", ["JPM", "V", "MA", "GS", "BAC", "BRK.B"]),
                    ], md=4),
                    dbc.Col([
                        _popular_category("Healthcare", ["UNH", "JNJ", "PFE", "MRK", "ABBV", "LLY"]),
                    ], md=4),
                ], className="mb-3"),
                dbc.Row([
                    dbc.Col([
                        _popular_category("Consumer", ["TSLA", "HD", "NKE", "MCD", "COST", "WMT"]),
                    ], md=4),
                    dbc.Col([
                        _popular_category("Energy & Industrial", ["XOM", "CVX", "HON", "UPS", "CAT"]),
                    ], md=4),
                    dbc.Col([
                        _popular_category("ETFs", ["SPY", "QQQ", "DIA", "IWM", "GLD", "TLT"]),
                    ], md=4),
                ]),
            ]),
        ])


def _popular_category(title: str, symbols: List[str]) -> dbc.Card:
    """Create a card showing a category of popular symbols"""
    return dbc.Card([
        dbc.CardHeader(html.H6(title, className="mb-0")),
        dbc.CardBody(
            html.Div([
                dbc.Badge(
                    sym,
                    id={"type": "popular-symbol-btn", "index": sym},
                    color="primary",
                    pill=True,
                    className="me-2 mb-2",
                    style={"cursor": "pointer", "fontSize": "0.9rem"},
                )
                for sym in symbols
            ])
        ),
    ], className="mb-3")
