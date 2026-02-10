import dash
from dash import dcc, html, Input, Output, State
import plotly.graph_objects as go
from plotly.subplots import make_subplots
import requests
import pandas as pd
from datetime import datetime
import dash_bootstrap_components as dbc
import os

# Initialize the Dash app
app = dash.Dash(
    __name__,
    external_stylesheets=[dbc.themes.DARKLY],
    suppress_callback_exceptions=True
)

# API Configuration
API_BASE_URL = "http://localhost:3000"
API_KEY = os.getenv("API_KEY", "") or os.getenv("API_KEYS", "").split(",")[0].strip()

# Warn if API key is not set
if not API_KEY:
    import warnings
    warnings.warn("API_KEY not set. Set API_KEY in .env file.", stacklevel=2)

# Headers for API requests
def get_headers():
    return {
        "X-API-Key": API_KEY,
        "Content-Type": "application/json"
    }

# App layout
app.layout = dbc.Container([
    # Header
    dbc.Row([
        dbc.Col([
            html.H1("🔬 InvestIQ - Validation Dashboard", className="text-center mb-4 mt-4"),
            html.P("Compare Our Analysis with Industry Standards & Run Backtests",
                   className="text-center text-muted mb-4")
        ])
    ]),

    # Tabs for different validation features
    dbc.Tabs([
        # Tab 1: Validation/Comparison
        dbc.Tab(label="📊 Data Validation", tab_id="validation-tab", children=[
            dbc.Container([
                # Search controls
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardBody([
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Stock Symbol"),
                                        dbc.Input(
                                            id='val-symbol-input',
                                            type='text',
                                            value='AAPL',
                                            placeholder='Enter symbol (e.g., AAPL)',
                                            className="mb-2"
                                        ),
                                    ], md=6),
                                    dbc.Col([
                                        dbc.Label(""),
                                        html.Br(),
                                        dbc.Button(
                                            "🔍 Validate",
                                            id='validate-button',
                                            color="primary",
                                            className="w-100",
                                            n_clicks=0
                                        ),
                                    ], md=6),
                                ])
                            ])
                        ], className="mb-4")
                    ])
                ], className="mb-4"),

                # Results section
                dbc.Row([
                    # Overall Accuracy Card
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Overall Accuracy"),
                            dbc.CardBody([
                                html.H2(id='overall-accuracy', className="text-center"),
                                html.P(id='accuracy-summary', className="text-center text-muted")
                            ])
                        ])
                    ], md=4),

                    # Technical Accuracy Card
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Technical Analysis Accuracy"),
                            dbc.CardBody([
                                html.H3(id='tech-accuracy', className="text-center"),
                                html.Div(id='tech-details')
                            ])
                        ])
                    ], md=4),

                    # Fundamental Accuracy Card
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Fundamental Analysis Accuracy"),
                            dbc.CardBody([
                                html.H3(id='fund-accuracy', className="text-center"),
                                html.Div(id='fund-details')
                            ])
                        ])
                    ], md=4),
                ], className="mb-4"),

                # Detailed comparison table
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Detailed Comparison"),
                            dbc.CardBody([
                                html.Div(id='comparison-table')
                            ])
                        ])
                    ])
                ], className="mb-4"),
            ], fluid=True)
        ]),

        # Tab 2: Backtesting
        dbc.Tab(label="📈 Backtesting", tab_id="backtest-tab", children=[
            dbc.Container([
                # Backtest controls
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardBody([
                                dbc.Row([
                                    dbc.Col([
                                        dbc.Label("Stock Symbol"),
                                        dbc.Input(
                                            id='bt-symbol-input',
                                            type='text',
                                            value='AAPL',
                                            placeholder='Enter symbol (e.g., AAPL)',
                                            className="mb-2"
                                        ),
                                    ], md=4),
                                    dbc.Col([
                                        dbc.Label("Days to Backtest"),
                                        dbc.Input(
                                            id='bt-days-input',
                                            type='number',
                                            value=365,
                                            min=90,
                                            max=730,
                                            className="mb-2"
                                        ),
                                    ], md=4),
                                    dbc.Col([
                                        dbc.Label(""),
                                        html.Br(),
                                        dbc.Button(
                                            "🚀 Run Backtest",
                                            id='backtest-button',
                                            color="success",
                                            className="w-100",
                                            n_clicks=0
                                        ),
                                    ], md=4),
                                ])
                            ])
                        ], className="mb-4")
                    ])
                ], className="mb-4"),

                # Performance Metrics
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Total Return"),
                            dbc.CardBody([
                                html.H2(id='total-return', className="text-center"),
                                html.P(id='return-pct', className="text-center text-muted")
                            ])
                        ])
                    ], md=3),
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Win Rate"),
                            dbc.CardBody([
                                html.H2(id='win-rate', className="text-center"),
                                html.P(id='trades-count', className="text-center text-muted")
                            ])
                        ])
                    ], md=3),
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Profit Factor"),
                            dbc.CardBody([
                                html.H2(id='profit-factor', className="text-center"),
                                html.P("Wins / Losses", className="text-center text-muted")
                            ])
                        ])
                    ], md=3),
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Sharpe Ratio"),
                            dbc.CardBody([
                                html.H2(id='sharpe-ratio', className="text-center"),
                                html.P("Risk-Adjusted Return", className="text-center text-muted")
                            ])
                        ])
                    ], md=3),
                ], className="mb-4"),

                # Equity Curve
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Equity Curve"),
                            dbc.CardBody([
                                dcc.Graph(id='equity-curve')
                            ])
                        ])
                    ])
                ], className="mb-4"),

                # Trade List
                dbc.Row([
                    dbc.Col([
                        dbc.Card([
                            dbc.CardHeader("Trade History"),
                            dbc.CardBody([
                                html.Div(id='trade-list')
                            ])
                        ])
                    ])
                ]),
            ], fluid=True)
        ]),
    ], id="tabs", active_tab="validation-tab"),

    # Loading spinner
    dcc.Loading(
        id="loading",
        type="default",
        children=html.Div(id="loading-output")
    )
], fluid=True)


# Validation callback
@app.callback(
    [
        Output('overall-accuracy', 'children'),
        Output('accuracy-summary', 'children'),
        Output('tech-accuracy', 'children'),
        Output('tech-details', 'children'),
        Output('fund-accuracy', 'children'),
        Output('fund-details', 'children'),
        Output('comparison-table', 'children'),
    ],
    [Input('validate-button', 'n_clicks')],
    [State('val-symbol-input', 'value')]
)
def validate_analysis(n_clicks, symbol):
    if n_clicks == 0:
        return ("N/A", "Click 'Validate' to start", "N/A", "", "N/A", "", "")

    if not symbol:
        return ("Error", "Please enter a symbol", "N/A", "", "N/A", "", "")

    try:
        # Call validation API
        response = requests.get(f'{API_BASE_URL}/api/validate/{symbol}', headers=get_headers())
        response.raise_for_status()
        data = response.json()

        if not data['success']:
            error_msg = data.get('error', 'Unknown error')
            return (
                "❌ Error",
                error_msg,
                "N/A", "", "N/A", "", ""
            )

        comparison = data['data']

        # Overall accuracy
        overall_acc = comparison['overall_accuracy']
        overall_text = f"{overall_acc:.1f}%"
        overall_color = "success" if overall_acc >= 80 else "warning" if overall_acc >= 60 else "danger"

        # Technical accuracy
        tech_comp = comparison['technical_comparison']
        tech_acc = tech_comp['overall_technical_accuracy']
        tech_text = f"{tech_acc:.1f}%"

        # Build technical details
        tech_details = []
        if tech_comp.get('rsi_difference'):
            rsi = tech_comp['rsi_difference']
            tech_details.append(html.P([
                html.Strong("RSI: "),
                f"Our: {rsi['our_value']:.1f}, ",
                f"Alpha Vantage: {rsi['their_value']:.1f} ",
                html.Span("✓", style={'color': 'green'}) if rsi['within_tolerance'] else
                html.Span(f"({rsi['percentage_difference']:+.1f}%)", style={'color': 'orange'})
            ], className="mb-1"))

        if tech_comp.get('sma_difference'):
            sma = tech_comp['sma_difference']
            tech_details.append(html.P([
                html.Strong("SMA-20: "),
                f"Our: ${sma['our_value']:.2f}, ",
                f"Alpha Vantage: ${sma['their_value']:.2f} ",
                html.Span("✓", style={'color': 'green'}) if sma['within_tolerance'] else
                html.Span(f"({sma['percentage_difference']:+.1f}%)", style={'color': 'orange'})
            ], className="mb-1"))

        # Fundamental accuracy
        fund_comp = comparison['fundamental_comparison']
        fund_acc = fund_comp['overall_fundamental_accuracy']
        fund_text = f"{fund_acc:.1f}%"

        # Build fundamental details
        fund_details = []
        if fund_comp.get('pe_ratio_difference'):
            pe = fund_comp['pe_ratio_difference']
            fund_details.append(html.P([
                html.Strong("P/E Ratio: "),
                f"Our: {pe['our_value']:.2f}, ",
                f"Yahoo: {pe['their_value']:.2f} ",
                html.Span("✓", style={'color': 'green'}) if pe['within_tolerance'] else
                html.Span(f"({pe['percentage_difference']:+.1f}%)", style={'color': 'orange'})
            ], className="mb-1"))

        if fund_comp.get('roe_difference'):
            roe = fund_comp['roe_difference']
            fund_details.append(html.P([
                html.Strong("ROE: "),
                f"Our: {roe['our_value']:.1f}%, ",
                f"Yahoo: {roe['their_value']:.1f}% ",
                html.Span("✓", style={'color': 'green'}) if roe['within_tolerance'] else
                html.Span(f"({roe['percentage_difference']:+.1f}%)", style={'color': 'orange'})
            ], className="mb-1"))

        # Summary
        summary_text = comparison['differences_summary']

        # Comparison table
        table_data = []
        table_data.append(html.Tr([
            html.Th("Metric"),
            html.Th("Our Value"),
            html.Th("External Source"),
            html.Th("Difference"),
            html.Th("Status")
        ]))

        # Add technical indicators
        if tech_comp.get('rsi_difference'):
            rsi = tech_comp['rsi_difference']
            table_data.append(html.Tr([
                html.Td("RSI"),
                html.Td(f"{rsi['our_value']:.2f}"),
                html.Td(f"{rsi['their_value']:.2f} (Alpha Vantage)"),
                html.Td(f"{rsi['percentage_difference']:+.2f}%"),
                html.Td("✅" if rsi['within_tolerance'] else "⚠️")
            ]))

        # Add fundamental metrics
        if fund_comp.get('pe_ratio_difference'):
            pe = fund_comp['pe_ratio_difference']
            table_data.append(html.Tr([
                html.Td("P/E Ratio"),
                html.Td(f"{pe['our_value']:.2f}"),
                html.Td(f"{pe['their_value']:.2f} (Yahoo Finance)"),
                html.Td(f"{pe['percentage_difference']:+.2f}%"),
                html.Td("✅" if pe['within_tolerance'] else "⚠️")
            ]))

        comparison_table = dbc.Table(table_data, bordered=True, hover=True, striped=True)

        return (
            overall_text,
            summary_text,
            tech_text,
            html.Div(tech_details),
            fund_text,
            html.Div(fund_details),
            comparison_table
        )

    except Exception as e:
        error_msg = str(e)
        return (
            "❌ Error",
            f"Validation failed: {error_msg}",
            "N/A", "", "N/A", "", ""
        )


# Backtest callback
@app.callback(
    [
        Output('total-return', 'children'),
        Output('return-pct', 'children'),
        Output('win-rate', 'children'),
        Output('trades-count', 'children'),
        Output('profit-factor', 'children'),
        Output('sharpe-ratio', 'children'),
        Output('equity-curve', 'figure'),
        Output('trade-list', 'children'),
    ],
    [Input('backtest-button', 'n_clicks')],
    [State('bt-symbol-input', 'value'), State('bt-days-input', 'value')]
)
def run_backtest(n_clicks, symbol, days):
    if n_clicks == 0:
        empty_fig = go.Figure()
        empty_fig.update_layout(template="plotly_dark")
        return ("N/A", "Click 'Run Backtest'", "N/A", "", "N/A", "N/A", empty_fig, "")

    if not symbol:
        empty_fig = go.Figure()
        empty_fig.update_layout(template="plotly_dark")
        return ("Error", "Please enter a symbol", "N/A", "", "N/A", "N/A", empty_fig, "")

    try:
        # Call backtest API
        response = requests.get(f'{API_BASE_URL}/api/backtest/{symbol}?days={days}', headers=get_headers())
        response.raise_for_status()
        data = response.json()

        if not data['success']:
            error_msg = data.get('error', 'Unknown error')
            empty_fig = go.Figure()
            empty_fig.update_layout(template="plotly_dark")
            return ("❌ Error", error_msg, "N/A", "", "N/A", "N/A", empty_fig, "")

        result = data['data']

        # Format metrics
        total_return = f"${result['total_return']:,.2f}"
        return_pct = f"{result['total_return_percent']:+.2f}%"
        win_rate_val = f"{result['win_rate']:.1f}%"
        trades_count = f"{result['winning_trades']}/{result['total_trades']} trades"
        profit_factor_val = f"{result['profit_factor']:.2f}"
        sharpe_val = f"{result['sharpe_ratio']:.2f}"

        # Create equity curve
        equity_data = result['equity_curve']
        timestamps = [point['timestamp'] for point in equity_data]
        equity_values = [point['equity'] for point in equity_data]

        fig = go.Figure()
        fig.add_trace(go.Scatter(
            x=timestamps,
            y=equity_values,
            mode='lines',
            name='Portfolio Value',
            line=dict(color='#00d4ff', width=2)
        ))

        # Add horizontal line for initial capital
        fig.add_hline(
            y=result['initial_capital'],
            line_dash="dash",
            line_color="gray",
            annotation_text="Initial Capital"
        )

        fig.update_layout(
            title=f"Equity Curve - {symbol}",
            xaxis_title="Date",
            yaxis_title="Portfolio Value ($)",
            template="plotly_dark",
            hovermode='x unified'
        )

        # Create trade list table
        trades = result['trades']
        if trades:
            trade_rows = [html.Tr([
                html.Th("Entry Date"),
                html.Th("Exit Date"),
                html.Th("Signal"),
                html.Th("Entry Price"),
                html.Th("Exit Price"),
                html.Th("P/L"),
                html.Th("P/L %"),
                html.Th("Days Held")
            ])]

            for trade in trades[:20]:  # Show last 20 trades
                pnl_color = "success" if trade['profit_loss'] > 0 else "danger"
                trade_rows.append(html.Tr([
                    html.Td(trade['entry_date'][:10]),
                    html.Td(trade['exit_date'][:10]),
                    html.Td(str(trade['signal'])),
                    html.Td(f"${trade['entry_price']:.2f}"),
                    html.Td(f"${trade['exit_price']:.2f}"),
                    html.Td(f"${trade['profit_loss']:+.2f}", style={'color': 'green' if trade['profit_loss'] > 0 else 'red'}),
                    html.Td(f"{trade['profit_loss_percent']:+.2f}%", style={'color': 'green' if trade['profit_loss'] > 0 else 'red'}),
                    html.Td(str(trade['holding_period_days']))
                ]))

            trade_table = dbc.Table(trade_rows, bordered=True, hover=True, striped=True, size="sm")
        else:
            trade_table = html.P("No trades executed in backtest period")

        return (
            total_return,
            return_pct,
            win_rate_val,
            trades_count,
            profit_factor_val,
            sharpe_val,
            fig,
            trade_table
        )

    except Exception as e:
        error_msg = str(e)
        empty_fig = go.Figure()
        empty_fig.update_layout(template="plotly_dark")
        return (
            "❌ Error",
            f"Backtest failed: {error_msg}",
            "N/A", "", "N/A", "N/A", empty_fig, ""
        )


if __name__ == '__main__':
    import os
    port = int(os.getenv('VALIDATION_DASHBOARD_PORT', 8051))
    app.run_server(debug=True, host='0.0.0.0', port=port)
