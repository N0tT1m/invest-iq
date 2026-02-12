"""Chart rendering via plotly + kaleido. Called from Rust via PyO3."""

import subprocess
import sys

def _ensure_deps():
    """Auto-install plotly and kaleido if missing."""
    for pkg, install_name in [("plotly", "plotly>=5.18.0"), ("kaleido", "kaleido==0.2.1")]:
        try:
            __import__(pkg)
        except ImportError:
            subprocess.check_call(
                [sys.executable, "-m", "pip", "install", "--break-system-packages", "-q", install_name],
            )

_ensure_deps()

import plotly.graph_objects as go
from plotly.subplots import make_subplots


def _ema(data, period):
    """Exponential moving average."""
    result = []
    mult = 2.0 / (period + 1)
    if len(data) < period:
        return result
    sma = sum(data[:period]) / period
    result.append(sma)
    for val in data[period:]:
        result.append((val - result[-1]) * mult + result[-1])
    return result


def _rsi(closes, period=14):
    """Relative Strength Index."""
    if len(closes) < period + 1:
        return [], []
    deltas = [closes[i] - closes[i - 1] for i in range(1, len(closes))]
    gains = [max(d, 0) for d in deltas]
    losses = [abs(min(d, 0)) for d in deltas]

    avg_gain = sum(gains[:period]) / period
    avg_loss = sum(losses[:period]) / period

    rsi_dates_idx = []
    rsi_vals = []
    for i in range(period, len(gains)):
        avg_gain = (avg_gain * (period - 1) + gains[i]) / period
        avg_loss = (avg_loss * (period - 1) + losses[i]) / period
        rs = avg_gain / avg_loss if avg_loss != 0 else 100.0
        rsi_vals.append(100.0 - 100.0 / (1.0 + rs))
        rsi_dates_idx.append(i + 1)  # +1 because deltas starts at index 1
    return rsi_dates_idx, rsi_vals


def _macd(closes, fast=12, slow=26, signal_period=9):
    """MACD line, signal line, histogram."""
    ema_fast = _ema(closes, fast)
    ema_slow = _ema(closes, slow)

    # Align: ema_fast starts at index fast-1, ema_slow at slow-1
    offset = slow - fast
    macd_line = [ema_fast[i + offset] - ema_slow[i] for i in range(len(ema_slow))]
    signal_line = _ema(macd_line, signal_period)

    sig_offset = signal_period - 1
    histogram = [macd_line[i + sig_offset] - signal_line[i] for i in range(len(signal_line))]

    # Start index in original data
    start_idx = slow - 1 + signal_period - 1
    return start_idx, macd_line, signal_line, histogram


def _sma(data, period):
    """Simple moving average."""
    if len(data) < period:
        return []
    return [sum(data[i - period:i]) / period for i in range(period, len(data) + 1)]


def _bollinger(closes, period=20, num_std=2.0):
    """Bollinger Bands."""
    if len(closes) < period:
        return [], [], []
    upper, middle, lower = [], [], []
    for i in range(period, len(closes) + 1):
        window = closes[i - period:i]
        mean = sum(window) / period
        variance = sum((x - mean) ** 2 for x in window) / period
        std = variance ** 0.5
        upper.append(mean + num_std * std)
        middle.append(mean)
        lower.append(mean - num_std * std)
    return upper, middle, lower


def render_chart(filename, title, signal_color, dates, opens, highs, lows, closes, volumes):
    """Generate a 4-panel chart: Candlestick+BB+SMA, Volume, RSI, MACD."""

    n = len(dates)
    if n == 0:
        return

    fig = make_subplots(
        rows=4, cols=1,
        shared_xaxes=True,
        vertical_spacing=0.03,
        row_heights=[0.50, 0.12, 0.19, 0.19],
        subplot_titles=[title, "Volume", "RSI (14)", "MACD"],
    )

    # --- Candlestick ---
    fig.add_trace(
        go.Candlestick(
            x=dates, open=opens, high=highs, low=lows, close=closes,
            increasing_line_color="#26A69A",
            decreasing_line_color="#EF5350",
            increasing_fillcolor="#26A69A",
            decreasing_fillcolor="#EF5350",
            name="Price",
            showlegend=False,
        ),
        row=1, col=1,
    )

    # Bollinger Bands
    bb_upper, bb_mid, bb_lower = _bollinger(closes, 20, 2.0)
    if bb_upper:
        bb_dates = dates[19:]
        fig.add_trace(go.Scatter(
            x=bb_dates, y=bb_upper, mode="lines",
            line=dict(color="rgba(100,149,237,0.4)", width=1),
            name="BB Upper", showlegend=False,
        ), row=1, col=1)
        fig.add_trace(go.Scatter(
            x=bb_dates, y=bb_lower, mode="lines",
            line=dict(color="rgba(100,149,237,0.4)", width=1),
            fill="tonexty", fillcolor="rgba(100,149,237,0.08)",
            name="BB Lower", showlegend=False,
        ), row=1, col=1)
        fig.add_trace(go.Scatter(
            x=bb_dates, y=bb_mid, mode="lines",
            line=dict(color="#6495ED", width=1, dash="dot"),
            name="SMA 20",
        ), row=1, col=1)

    # SMA 50
    sma50 = _sma(closes, 50)
    if sma50:
        fig.add_trace(go.Scatter(
            x=dates[49:], y=sma50, mode="lines",
            line=dict(color="#FF6FFF", width=1.5),
            name="SMA 50",
        ), row=1, col=1)

    # --- Volume ---
    vol_colors = [
        "#26A69A" if closes[i] >= opens[i] else "#EF5350"
        for i in range(n)
    ]
    fig.add_trace(
        go.Bar(
            x=dates, y=volumes,
            marker_color=vol_colors, opacity=0.6,
            name="Volume", showlegend=False,
        ),
        row=2, col=1,
    )

    # Volume SMA 20
    vol_sma = _sma(volumes, 20)
    if vol_sma:
        fig.add_trace(go.Scatter(
            x=dates[19:], y=vol_sma, mode="lines",
            line=dict(color="#42A5F5", width=1),
            name="Vol SMA", showlegend=False,
        ), row=2, col=1)

    # --- RSI ---
    rsi_idx, rsi_vals = _rsi(closes, 14)
    if rsi_vals:
        rsi_dates = [dates[i] for i in rsi_idx]
        fig.add_trace(go.Scatter(
            x=rsi_dates, y=rsi_vals, mode="lines",
            line=dict(color="#00BCD4", width=1.5),
            name="RSI", showlegend=False,
        ), row=3, col=1)

        # Overbought / oversold bands
        fig.add_hline(y=70, line_dash="dash", line_color="rgba(255,23,68,0.4)", row=3, col=1)
        fig.add_hline(y=30, line_dash="dash", line_color="rgba(0,200,83,0.4)", row=3, col=1)
        fig.add_hline(y=50, line_dash="dot", line_color="rgba(255,255,255,0.2)", row=3, col=1)

        # Shade overbought/oversold zones
        fig.add_hrect(y0=70, y1=100, fillcolor="rgba(255,23,68,0.06)",
                      line_width=0, row=3, col=1)
        fig.add_hrect(y0=0, y1=30, fillcolor="rgba(0,200,83,0.06)",
                      line_width=0, row=3, col=1)

    # --- MACD ---
    start_idx, macd_line, signal_line, histogram = _macd(closes, 12, 26, 9)
    if histogram:
        hist_dates = dates[start_idx: start_idx + len(histogram)]
        macd_dates = dates[25: 25 + len(macd_line)]
        sig_dates = dates[start_idx: start_idx + len(signal_line)]

        hist_colors = ["#26A69A" if v >= 0 else "#EF5350" for v in histogram]
        fig.add_trace(go.Bar(
            x=hist_dates, y=histogram,
            marker_color=hist_colors, opacity=0.5,
            name="Histogram", showlegend=False,
        ), row=4, col=1)

        fig.add_trace(go.Scatter(
            x=macd_dates, y=macd_line, mode="lines",
            line=dict(color="#42A5F5", width=1.5),
            name="MACD", showlegend=False,
        ), row=4, col=1)

        fig.add_trace(go.Scatter(
            x=sig_dates, y=signal_line, mode="lines",
            line=dict(color="#FFA726", width=1.5),
            name="Signal", showlegend=False,
        ), row=4, col=1)

        fig.add_hline(y=0, line_dash="dot", line_color="rgba(255,255,255,0.2)", row=4, col=1)

    # --- Layout ---
    fig.update_layout(
        template="plotly_dark",
        paper_bgcolor="#1a1a2e",
        plot_bgcolor="#16213e",
        font=dict(family="Arial", size=12, color="#e0e0e0"),
        title=dict(
            text=f"<b>{title}</b>",
            font=dict(size=22, color=signal_color),
            x=0.5,
        ),
        legend=dict(
            orientation="h", yanchor="bottom", y=1.02, xanchor="right", x=1,
            font=dict(size=10),
        ),
        width=1400,
        height=1000,
        margin=dict(l=70, r=30, t=80, b=40),
        xaxis_rangeslider_visible=False,
    )

    # Style axes
    for i in range(1, 5):
        xaxis = f"xaxis{i}" if i > 1 else "xaxis"
        yaxis = f"yaxis{i}" if i > 1 else "yaxis"
        fig.update_layout(**{
            f"{xaxis}_gridcolor": "rgba(255,255,255,0.06)",
            f"{yaxis}_gridcolor": "rgba(255,255,255,0.06)",
        })

    fig.update_yaxes(title_text="Price ($)", row=1, col=1)
    fig.update_yaxes(row=3, col=1, range=[0, 100])

    # Subtitle styling
    for ann in fig.layout.annotations:
        ann.font.size = 14
        ann.font.color = "#aaaaaa"

    fig.write_image(filename, scale=2)
