"""
Helper utilities for enhanced InvestIQ dashboard
"""

import plotly.graph_objects as go
from plotly.subplots import make_subplots
import pandas as pd
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional
import dash_bootstrap_components as dbc
from dash import html


class ChartEnhancer:
    """Enhanced chart creation with better interactivity"""

    @staticmethod
    def create_enhanced_candlestick(
        df: pd.DataFrame,
        symbol: str,
        show_volume: bool = True,
        show_bb: bool = True,
        show_ma: bool = True,
        height: int = 600
    ) -> go.Figure:
        """Create an enhanced candlestick chart with multiple indicators"""

        rows = 2 if show_volume else 1
        row_heights = [0.7, 0.3] if show_volume else [1.0]

        fig = make_subplots(
            rows=rows,
            cols=1,
            shared_xaxes=True,
            vertical_spacing=0.03,
            row_heights=row_heights,
            subplot_titles=(f'{symbol} Price', 'Volume') if show_volume else (f'{symbol} Price',)
        )

        # Candlestick
        fig.add_trace(
            go.Candlestick(
                x=df['timestamp'],
                open=df['open'],
                high=df['high'],
                low=df['low'],
                close=df['close'],
                name='Price',
                increasing_line_color='#38ef7d',
                decreasing_line_color='#f45c43',
                increasing_fillcolor='rgba(56, 239, 125, 0.3)',
                decreasing_fillcolor='rgba(244, 92, 67, 0.3)',
            ),
            row=1, col=1
        )

        # Bollinger Bands
        if show_bb and len(df) >= 20:
            sma_20 = df['close'].rolling(window=20).mean()
            std_20 = df['close'].rolling(window=20).std()
            upper_band = sma_20 + (std_20 * 2)
            lower_band = sma_20 - (std_20 * 2)

            fig.add_trace(
                go.Scatter(
                    x=df['timestamp'],
                    y=upper_band,
                    name='BB Upper',
                    line=dict(color='rgba(102, 126, 234, 0.3)', width=1, dash='dot'),
                    showlegend=True
                ),
                row=1, col=1
            )

            fig.add_trace(
                go.Scatter(
                    x=df['timestamp'],
                    y=sma_20,
                    name='SMA 20',
                    line=dict(color='#667eea', width=2),
                    showlegend=True
                ),
                row=1, col=1
            )

            fig.add_trace(
                go.Scatter(
                    x=df['timestamp'],
                    y=lower_band,
                    name='BB Lower',
                    line=dict(color='rgba(102, 126, 234, 0.3)', width=1, dash='dot'),
                    fill='tonexty',
                    fillcolor='rgba(102, 126, 234, 0.05)',
                    showlegend=True
                ),
                row=1, col=1
            )

        # Moving Averages
        if show_ma and len(df) >= 50:
            sma_50 = df['close'].rolling(window=50).mean()
            sma_200 = df['close'].rolling(window=200).mean() if len(df) >= 200 else None

            fig.add_trace(
                go.Scatter(
                    x=df['timestamp'],
                    y=sma_50,
                    name='SMA 50',
                    line=dict(color='#f5576c', width=2, dash='dash'),
                    showlegend=True
                ),
                row=1, col=1
            )

            if sma_200 is not None:
                fig.add_trace(
                    go.Scatter(
                        x=df['timestamp'],
                        y=sma_200,
                        name='SMA 200',
                        line=dict(color='#00f2fe', width=2, dash='dash'),
                        showlegend=True
                    ),
                    row=1, col=1
                )

        # Volume bars
        if show_volume:
            colors = ['#f45c43' if df['close'].iloc[i] < df['open'].iloc[i] else '#38ef7d'
                      for i in range(len(df))]

            fig.add_trace(
                go.Bar(
                    x=df['timestamp'],
                    y=df['volume'],
                    name='Volume',
                    marker_color=colors,
                    showlegend=False,
                    opacity=0.7
                ),
                row=2 if show_volume else 1, col=1
            )

        # Enhanced layout
        fig.update_layout(
            height=height,
            template='plotly_dark',
            xaxis_rangeslider_visible=False,
            hovermode='x unified',
            hoverlabel=dict(
                bgcolor="rgba(26, 31, 58, 0.95)",
                font_size=12,
                font_family="Arial"
            ),
            legend=dict(
                orientation="h",
                yanchor="bottom",
                y=1.02,
                xanchor="right",
                x=1,
                bgcolor="rgba(26, 31, 58, 0.8)",
                bordercolor="rgba(102, 126, 234, 0.3)",
                borderwidth=1
            ),
            margin=dict(l=50, r=50, t=50, b=50),
            paper_bgcolor='rgba(0,0,0,0)',
            plot_bgcolor='rgba(0,0,0,0)',
        )

        # Enhanced axes
        fig.update_xaxes(
            title_text="Date",
            row=rows, col=1,
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)',
            showline=True,
            linewidth=2,
            linecolor='rgba(102, 126, 234, 0.3)',
        )

        fig.update_yaxes(
            title_text="Price ($)",
            row=1, col=1,
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)',
            showline=True,
            linewidth=2,
            linecolor='rgba(102, 126, 234, 0.3)',
        )

        if show_volume:
            fig.update_yaxes(
                title_text="Volume",
                row=2, col=1,
                showgrid=True,
                gridwidth=1,
                gridcolor='rgba(255, 255, 255, 0.05)',
            )

        # Add range selector buttons
        fig.update_xaxes(
            rangeselector=dict(
                buttons=list([
                    dict(count=1, label="1D", step="day", stepmode="backward"),
                    dict(count=7, label="1W", step="day", stepmode="backward"),
                    dict(count=1, label="1M", step="month", stepmode="backward"),
                    dict(count=3, label="3M", step="month", stepmode="backward"),
                    dict(count=6, label="6M", step="month", stepmode="backward"),
                    dict(count=1, label="1Y", step="year", stepmode="backward"),
                    dict(step="all", label="All")
                ]),
                bgcolor="rgba(102, 126, 234, 0.1)",
                activecolor="rgba(102, 126, 234, 0.3)",
                font=dict(color="white"),
                x=0,
                y=1.1
            )
        )

        return fig

    @staticmethod
    def create_enhanced_rsi(df: pd.DataFrame, analysis: Dict = None) -> go.Figure:
        """Create enhanced RSI chart with zones and signals"""
        if len(df) < 14:
            return ChartEnhancer._create_empty_figure("Insufficient data for RSI")

        # Calculate RSI
        delta = df['close'].diff()
        gain = (delta.where(delta > 0, 0)).rolling(window=14).mean()
        loss = (-delta.where(delta < 0, 0)).rolling(window=14).mean()
        rs = gain / loss
        rsi = 100 - (100 / (1 + rs))

        fig = go.Figure()

        # RSI line
        fig.add_trace(go.Scatter(
            x=df['timestamp'],
            y=rsi,
            mode='lines',
            name='RSI',
            line=dict(color='#00f2fe', width=2),
            fill='tozeroy',
            fillcolor='rgba(0, 242, 254, 0.1)'
        ))

        # Reference zones
        fig.add_hrect(
            y0=70, y1=100,
            fillcolor="rgba(244, 92, 67, 0.2)",
            layer="below",
            line_width=0,
            annotation_text="Overbought",
            annotation_position="top right"
        )

        fig.add_hrect(
            y0=0, y1=30,
            fillcolor="rgba(56, 239, 125, 0.2)",
            layer="below",
            line_width=0,
            annotation_text="Oversold",
            annotation_position="bottom right"
        )

        # Reference lines
        fig.add_hline(y=70, line_dash="dash", line_color="rgba(244, 92, 67, 0.5)", line_width=2)
        fig.add_hline(y=30, line_dash="dash", line_color="rgba(56, 239, 125, 0.5)", line_width=2)
        fig.add_hline(y=50, line_dash="dot", line_color="rgba(255, 255, 255, 0.3)", line_width=1)

        # Highlight current RSI
        if analysis and analysis.get('technical', {}).get('metrics', {}).get('rsi'):
            current_rsi = analysis['technical']['metrics']['rsi']
            fig.add_annotation(
                x=df['timestamp'].iloc[-1],
                y=current_rsi,
                text=f"<b>RSI: {current_rsi:.1f}</b>",
                showarrow=True,
                arrowhead=2,
                arrowsize=1,
                arrowwidth=2,
                arrowcolor="#00f2fe",
                bgcolor="rgba(0, 242, 254, 0.8)",
                bordercolor="#00f2fe",
                borderwidth=2,
                font=dict(color="black", size=12, family="Arial Black")
            )

        fig.update_layout(
            height=300,
            template='plotly_dark',
            yaxis_title="RSI",
            yaxis=dict(range=[0, 100]),
            hovermode='x unified',
            showlegend=False,
            margin=dict(l=50, r=50, t=30, b=50),
            paper_bgcolor='rgba(0,0,0,0)',
            plot_bgcolor='rgba(0,0,0,0)',
        )

        fig.update_xaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        fig.update_yaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        return fig

    @staticmethod
    def create_enhanced_macd(df: pd.DataFrame, analysis: Dict = None) -> go.Figure:
        """Create enhanced MACD chart with histogram"""
        if len(df) < 26:
            return ChartEnhancer._create_empty_figure("Insufficient data for MACD")

        # Calculate MACD
        exp1 = df['close'].ewm(span=12, adjust=False).mean()
        exp2 = df['close'].ewm(span=26, adjust=False).mean()
        macd = exp1 - exp2
        signal = macd.ewm(span=9, adjust=False).mean()
        histogram = macd - signal

        fig = go.Figure()

        # MACD line
        fig.add_trace(go.Scatter(
            x=df['timestamp'],
            y=macd,
            mode='lines',
            name='MACD',
            line=dict(color='#667eea', width=2)
        ))

        # Signal line
        fig.add_trace(go.Scatter(
            x=df['timestamp'],
            y=signal,
            mode='lines',
            name='Signal',
            line=dict(color='#f5576c', width=2)
        ))

        # Histogram with gradient colors
        colors = ['rgba(56, 239, 125, 0.6)' if val >= 0 else 'rgba(244, 92, 67, 0.6)'
                  for val in histogram]

        fig.add_trace(go.Bar(
            x=df['timestamp'],
            y=histogram,
            name='Histogram',
            marker_color=colors,
            marker_line_width=0
        ))

        # Zero line
        fig.add_hline(y=0, line_dash="dash", line_color="rgba(255, 255, 255, 0.3)", line_width=2)

        # Crossover annotations
        for i in range(1, len(macd)):
            if macd.iloc[i-1] < signal.iloc[i-1] and macd.iloc[i] > signal.iloc[i]:
                # Bullish crossover
                fig.add_annotation(
                    x=df['timestamp'].iloc[i],
                    y=macd.iloc[i],
                    text="▲",
                    showarrow=False,
                    font=dict(size=20, color="#38ef7d")
                )
            elif macd.iloc[i-1] > signal.iloc[i-1] and macd.iloc[i] < signal.iloc[i]:
                # Bearish crossover
                fig.add_annotation(
                    x=df['timestamp'].iloc[i],
                    y=macd.iloc[i],
                    text="▼",
                    showarrow=False,
                    font=dict(size=20, color="#f45c43")
                )

        fig.update_layout(
            height=300,
            template='plotly_dark',
            yaxis_title="MACD",
            hovermode='x unified',
            legend=dict(
                orientation="h",
                yanchor="bottom",
                y=1.02,
                xanchor="right",
                x=1
            ),
            margin=dict(l=50, r=50, t=30, b=50),
            paper_bgcolor='rgba(0,0,0,0)',
            plot_bgcolor='rgba(0,0,0,0)',
        )

        fig.update_xaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        fig.update_yaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        return fig

    @staticmethod
    def create_comparison_chart(symbols_data: List[Dict], timeframe: str) -> go.Figure:
        """Create a comparison chart for multiple stocks"""
        fig = go.Figure()

        colors = ['#667eea', '#38ef7d', '#f5576c', '#00f2fe', '#f093fb']

        for idx, stock_data in enumerate(symbols_data):
            symbol = stock_data['symbol']
            df = pd.DataFrame(stock_data['bars'])
            df['timestamp'] = pd.to_datetime(df['timestamp'])

            # Normalize to percentage change
            df['pct_change'] = ((df['close'] - df['close'].iloc[0]) / df['close'].iloc[0]) * 100

            fig.add_trace(go.Scatter(
                x=df['timestamp'],
                y=df['pct_change'],
                mode='lines',
                name=symbol,
                line=dict(color=colors[idx % len(colors)], width=3),
                hovertemplate=f'{symbol}<br>%{{y:.2f}}%<extra></extra>'
            ))

        fig.update_layout(
            height=500,
            template='plotly_dark',
            title='Stock Comparison (Normalized % Change)',
            yaxis_title="% Change",
            xaxis_title="Date",
            hovermode='x unified',
            legend=dict(
                orientation="h",
                yanchor="bottom",
                y=1.02,
                xanchor="right",
                x=1,
                bgcolor="rgba(26, 31, 58, 0.8)",
                bordercolor="rgba(102, 126, 234, 0.3)",
                borderwidth=1
            ),
            paper_bgcolor='rgba(0,0,0,0)',
            plot_bgcolor='rgba(0,0,0,0)',
        )

        fig.update_xaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        fig.update_yaxes(
            showgrid=True,
            gridwidth=1,
            gridcolor='rgba(255, 255, 255, 0.05)'
        )

        return fig

    @staticmethod
    def _create_empty_figure(message: str) -> go.Figure:
        """Create empty figure with message"""
        fig = go.Figure()
        fig.add_annotation(
            text=message,
            xref="paper",
            yref="paper",
            x=0.5,
            y=0.5,
            showarrow=False,
            font=dict(size=16, color="gray")
        )
        fig.update_layout(
            template='plotly_dark',
            xaxis=dict(showgrid=False, showticklabels=False),
            yaxis=dict(showgrid=False, showticklabels=False),
            paper_bgcolor='rgba(0,0,0,0)',
            plot_bgcolor='rgba(0,0,0,0)',
        )
        return fig


class MessageEnhancer:
    """Better error and success messages"""

    @staticmethod
    def create_error_message(error: str, suggestion: str = None) -> dbc.Alert:
        """Create user-friendly error message"""
        content = [
            html.H5([html.I(className="fas fa-exclamation-triangle me-2"), "Oops! Something went wrong"], className="alert-heading"),
            html.P(error, className="mb-2"),
        ]

        if suggestion:
            content.append(html.Hr())
            content.append(html.P([
                html.Strong("💡 Try this: "),
                suggestion
            ], className="mb-0 small"))

        return dbc.Alert(content, color="danger", dismissable=True, className="fade-in")

    @staticmethod
    def create_success_message(message: str, details: str = None) -> dbc.Alert:
        """Create success message"""
        content = [
            html.H5([html.I(className="fas fa-check-circle me-2"), "Success!"], className="alert-heading"),
            html.P(message, className="mb-0"),
        ]

        if details:
            content.append(html.P(details, className="mt-2 small text-muted"))

        return dbc.Alert(content, color="success", dismissable=True, duration=5000, className="fade-in")

    @staticmethod
    def create_info_message(title: str, message: str) -> dbc.Alert:
        """Create info message"""
        return dbc.Alert([
            html.H5([html.I(className="fas fa-info-circle me-2"), title], className="alert-heading"),
            html.P(message, className="mb-0"),
        ], color="info", dismissable=True, className="fade-in")

    @staticmethod
    def create_warning_message(message: str) -> dbc.Alert:
        """Create warning message"""
        return dbc.Alert([
            html.I(className="fas fa-exclamation-triangle me-2"),
            message
        ], color="warning", dismissable=True, className="fade-in")


class SymbolAutocomplete:
    """Symbol autocomplete and search functionality"""

    # Common stock symbols for autocomplete
    POPULAR_SYMBOLS = [
        {'label': 'AAPL - Apple Inc.', 'value': 'AAPL'},
        {'label': 'MSFT - Microsoft Corporation', 'value': 'MSFT'},
        {'label': 'GOOGL - Alphabet Inc.', 'value': 'GOOGL'},
        {'label': 'AMZN - Amazon.com Inc.', 'value': 'AMZN'},
        {'label': 'META - Meta Platforms Inc.', 'value': 'META'},
        {'label': 'TSLA - Tesla Inc.', 'value': 'TSLA'},
        {'label': 'NVDA - NVIDIA Corporation', 'value': 'NVDA'},
        {'label': 'JPM - JPMorgan Chase & Co.', 'value': 'JPM'},
        {'label': 'V - Visa Inc.', 'value': 'V'},
        {'label': 'WMT - Walmart Inc.', 'value': 'WMT'},
        {'label': 'JNJ - Johnson & Johnson', 'value': 'JNJ'},
        {'label': 'PG - Procter & Gamble Co.', 'value': 'PG'},
        {'label': 'MA - Mastercard Inc.', 'value': 'MA'},
        {'label': 'DIS - The Walt Disney Company', 'value': 'DIS'},
        {'label': 'NFLX - Netflix Inc.', 'value': 'NFLX'},
        {'label': 'BAC - Bank of America Corp.', 'value': 'BAC'},
        {'label': 'ADBE - Adobe Inc.', 'value': 'ADBE'},
        {'label': 'CRM - Salesforce Inc.', 'value': 'CRM'},
        {'label': 'CSCO - Cisco Systems Inc.', 'value': 'CSCO'},
        {'label': 'INTC - Intel Corporation', 'value': 'INTC'},
    ]

    @staticmethod
    def get_symbol_options(recent_symbols: List[str] = None, watchlist: List[str] = None) -> List[Dict]:
        """Get autocomplete options with recent and watchlist"""
        options = []

        # Add recent symbols
        if recent_symbols:
            options.append({'label': '--- Recently Viewed ---', 'value': '', 'disabled': True})
            for symbol in recent_symbols[:5]:
                options.append({'label': f'🕐 {symbol}', 'value': symbol})

        # Add watchlist
        if watchlist:
            options.append({'label': '--- Watchlist ---', 'value': '', 'disabled': True})
            for symbol in watchlist:
                options.append({'label': f'⭐ {symbol}', 'value': symbol})

        # Add popular symbols
        options.append({'label': '--- Popular Stocks ---', 'value': '', 'disabled': True})
        options.extend(SymbolAutocomplete.POPULAR_SYMBOLS)

        return options


class ExportHelper:
    """Export functionality for data"""

    @staticmethod
    def export_analysis_to_csv(analysis: Dict, symbol: str) -> str:
        """Convert analysis to CSV format"""
        import io
        import csv

        output = io.StringIO()
        writer = csv.writer(output)

        writer.writerow(['InvestIQ Analysis Report', symbol, datetime.now().strftime('%Y-%m-%d %H:%M:%S')])
        writer.writerow([])

        # Overall signal
        writer.writerow(['Overall Signal', analysis.get('overall_signal', 'N/A')])
        writer.writerow(['Confidence', f"{analysis.get('overall_confidence', 0) * 100:.1f}%"])
        writer.writerow([])

        # Technical Analysis
        if 'technical' in analysis:
            tech = analysis['technical']
            writer.writerow(['Technical Analysis'])
            writer.writerow(['Signal', tech.get('signal', 'N/A')])
            writer.writerow(['Confidence', f"{tech.get('confidence', 0) * 100:.1f}%"])
            if 'metrics' in tech:
                for key, value in tech['metrics'].items():
                    writer.writerow([key, value])
            writer.writerow([])

        # Add other sections similarly...

        return output.getvalue()

    @staticmethod
    def export_analysis_to_json(analysis: Dict) -> str:
        """Convert analysis to JSON format"""
        import json
        return json.dumps(analysis, indent=2)
