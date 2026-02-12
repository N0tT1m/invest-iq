"""PatchTST Training Script."""
import os
from dotenv import load_dotenv
load_dotenv()                       # ml-services/.env
load_dotenv(dotenv_path="../.env")  # project root .env
import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader
import numpy as np
import pandas as pd
from pathlib import Path
import argparse
import sys
from loguru import logger
import json
from tqdm import tqdm
import requests
from datetime import datetime, timedelta
from typing import Dict, List, Tuple, Optional

sys.path.append(str(Path(__file__).parent.parent))
from shared.config import config
from shared.database import MLDatabase
from shared.polygon import fetch_active_tickers
from price_predictor.model import PatchTST, create_model

try:
    import invest_iq_data
    _USE_RUST_FETCHER = True
except ImportError:
    _USE_RUST_FETCHER = False

try:
    import sqlite3
    _HAS_SQLITE = True
except ImportError:
    _HAS_SQLITE = False


class TimeSeriesDataset(Dataset):
    """Dataset for time series forecasting."""

    def __init__(
        self,
        data: np.ndarray,
        context_length: int,
        prediction_length: int,
        stride: int = 1
    ):
        """
        Args:
            data: (num_samples, num_features) array
            context_length: Length of input sequence
            prediction_length: Length of prediction
            stride: Stride for sliding window
        """
        self.data = data
        self.context_length = context_length
        self.prediction_length = prediction_length
        self.stride = stride

        # Create indices
        self.indices = []
        for i in range(0, len(data) - context_length - prediction_length + 1, stride):
            self.indices.append(i)

    def __len__(self):
        return len(self.indices)

    def __getitem__(self, idx):
        start_idx = self.indices[idx]
        end_idx = start_idx + self.context_length
        pred_end = end_idx + self.prediction_length

        # Input sequence
        x = self.data[start_idx:end_idx].copy()

        # Target sequence
        y_price = self.data[end_idx:pred_end, 3].copy()  # Close price (index 3)

        # Direction labels (0=up, 1=down, 2=neutral)
        y_direction = np.zeros(self.prediction_length, dtype=np.long)
        threshold = 0.001  # 0.1% threshold for neutral

        for i in range(self.prediction_length):
            if i == 0:
                current = self.data[end_idx - 1, 3]
            else:
                current = self.data[end_idx + i - 1, 3]
            future = self.data[end_idx + i, 3]

            change_pct = (future - current) / (current + 1e-8)

            if change_pct > threshold:
                y_direction[i] = 0  # up
            elif change_pct < -threshold:
                y_direction[i] = 1  # down
            else:
                y_direction[i] = 2  # neutral

        return (
            torch.from_numpy(x).float(),
            torch.from_numpy(y_price).float(),
            torch.from_numpy(y_direction).long()
        )


class PatchTSTTrainer:
    """Trainer for PatchTST model."""

    def __init__(
        self,
        model: PatchTST,
        device: str = "cuda",
        learning_rate: float = 1e-4,
        weight_decay: float = 1e-5
    ):
        self.model = model.to(device)
        self.device = device

        self.optimizer = optim.AdamW(
            model.parameters(),
            lr=learning_rate,
            weight_decay=weight_decay
        )

        self.scheduler = optim.lr_scheduler.ReduceLROnPlateau(
            self.optimizer,
            mode='min',
            factor=0.5,
            patience=5,
        )

        # Loss functions
        self.price_criterion = nn.MSELoss()
        self.direction_criterion = nn.CrossEntropyLoss()

        self.best_val_loss = float('inf')

        # Mixed precision training for CUDA (GradScaler not supported on MPS/CPU)
        self.use_amp = device == "cuda"
        self.scaler = torch.amp.GradScaler("cuda") if self.use_amp else None

    def train_epoch(self, train_loader: DataLoader) -> Tuple[float, float, float]:
        """Train for one epoch. Uses mixed precision on CUDA for ~2x speedup."""
        self.model.train()

        total_loss = 0
        total_price_loss = 0
        total_direction_loss = 0
        num_batches = 0

        pbar = tqdm(train_loader, desc="Training")
        for x, y_price, y_direction in pbar:
            x = x.to(self.device, non_blocking=True)
            y_price = y_price.to(self.device, non_blocking=True)
            y_direction = y_direction.to(self.device, non_blocking=True)

            self.optimizer.zero_grad()

            # Forward pass with optional mixed precision (autocast)
            with torch.amp.autocast("cuda", enabled=self.use_amp):
                price_pred, direction_logits = self.model(x)
                price_loss = self.price_criterion(price_pred, y_price)
                direction_logits_flat = direction_logits.reshape(-1, 3)
                y_direction_flat = y_direction.reshape(-1)
                direction_loss = self.direction_criterion(direction_logits_flat, y_direction_flat)
                loss = price_loss + 0.5 * direction_loss

            # Backward pass with GradScaler for mixed precision
            if self.scaler is not None:
                self.scaler.scale(loss).backward()
                self.scaler.unscale_(self.optimizer)
                torch.nn.utils.clip_grad_norm_(self.model.parameters(), 1.0)
                self.scaler.step(self.optimizer)
                self.scaler.update()
            else:
                loss.backward()
                torch.nn.utils.clip_grad_norm_(self.model.parameters(), 1.0)
                self.optimizer.step()

            # Accumulate losses
            total_loss += loss.item()
            total_price_loss += price_loss.item()
            total_direction_loss += direction_loss.item()
            num_batches += 1

            pbar.set_postfix({
                'loss': f'{loss.item():.4f}',
                'price': f'{price_loss.item():.4f}',
                'dir': f'{direction_loss.item():.4f}'
            })

        return (
            total_loss / num_batches,
            total_price_loss / num_batches,
            total_direction_loss / num_batches
        )

    @torch.no_grad()
    def validate(self, val_loader: DataLoader) -> Tuple[float, float, float, float]:
        """Validate model."""
        self.model.eval()

        total_loss = 0
        total_price_loss = 0
        total_direction_loss = 0
        correct_direction = 0
        total_predictions = 0
        num_batches = 0

        for x, y_price, y_direction in val_loader:
            x = x.to(self.device, non_blocking=True)
            y_price = y_price.to(self.device, non_blocking=True)
            y_direction = y_direction.to(self.device, non_blocking=True)

            # Forward pass
            price_pred, direction_logits = self.model(x)

            # Compute losses
            price_loss = self.price_criterion(price_pred, y_price)

            direction_logits_flat = direction_logits.reshape(-1, 3)
            y_direction_flat = y_direction.reshape(-1)
            direction_loss = self.direction_criterion(direction_logits_flat, y_direction_flat)

            loss = price_loss + 0.5 * direction_loss

            # Compute accuracy
            direction_pred = torch.argmax(direction_logits_flat, dim=1)
            correct_direction += (direction_pred == y_direction_flat).sum().item()
            total_predictions += y_direction_flat.numel()

            total_loss += loss.item()
            total_price_loss += price_loss.item()
            total_direction_loss += direction_loss.item()
            num_batches += 1

        accuracy = correct_direction / total_predictions

        return (
            total_loss / num_batches,
            total_price_loss / num_batches,
            total_direction_loss / num_batches,
            accuracy
        )

    def save_checkpoint(self, path: Path, epoch: int, val_loss: float, metrics: dict):
        """Save model checkpoint."""
        checkpoint = {
            'epoch': epoch,
            'model_state_dict': self.model.state_dict(),
            'optimizer_state_dict': self.optimizer.state_dict(),
            'val_loss': val_loss,
            'metrics': metrics
        }
        torch.save(checkpoint, path)
        logger.info(f"Saved checkpoint to {path}")


def _polygon_timespan(interval: str) -> Tuple[int, str]:
    """Map user interval string to Polygon (multiplier, timespan)."""
    mapping = {
        "1m": (1, "minute"), "5m": (5, "minute"),
        "15m": (15, "minute"), "1h": (1, "hour"),
        "1d": (1, "day"),
    }
    return mapping.get(interval, (15, "minute"))


def _fetch_polygon_bars(
    symbol: str, start_date: str, end_date: str, interval: str, api_key: str
) -> pd.DataFrame:
    """Fetch OHLCV bars from Polygon REST API."""
    mult, span = _polygon_timespan(interval)
    url = (
        f"https://api.polygon.io/v2/aggs/ticker/{symbol}/range"
        f"/{mult}/{span}/{start_date}/{end_date}"
        f"?adjusted=true&sort=asc&limit=50000&apiKey={api_key}"
    )
    resp = requests.get(url, timeout=30)
    resp.raise_for_status()
    data = resp.json()
    results = data.get("results", [])
    if not results:
        return pd.DataFrame()

    df = pd.DataFrame(results)
    df = df.rename(columns={
        "o": "Open", "h": "High", "l": "Low", "c": "Close",
        "v": "Volume", "vw": "vwap", "t": "timestamp",
    })
    df.index = pd.to_datetime(df["timestamp"], unit="ms")
    df["symbol"] = symbol
    if "vwap" not in df.columns:
        df["vwap"] = (df["High"] + df["Low"] + df["Close"]) / 3
    return df[["Open", "High", "Low", "Close", "Volume", "vwap", "symbol"]]


def _fetch_yfinance_bars(
    symbol: str, start_date: str, end_date: str, interval: str
) -> pd.DataFrame:
    """Fetch OHLCV bars from yfinance (fallback)."""
    import yfinance as yf

    ticker = yf.Ticker(symbol)
    df = ticker.history(start=start_date, end=end_date, interval=interval)
    if len(df) > 0:
        df["symbol"] = symbol
        df["vwap"] = (df["High"] + df["Low"] + df["Close"]) / 3
    return df


def _fetch_bars_via_rust(
    symbols: List[str],
    days: int,
    interval: str,
) -> pd.DataFrame:
    """Fetch bars concurrently using the Rust invest_iq_data module."""
    polygon_key = os.environ.get("POLYGON_API_KEY", "")
    # Map interval to Polygon timespan
    timespan_map = {"1m": "minute", "5m": "minute", "15m": "minute", "1h": "hour", "1d": "day"}
    timespan = timespan_map.get(interval, "day")

    logger.info(f"Using Rust fetcher for {len(symbols)} symbols ({days}d, {timespan})")
    raw = invest_iq_data.fetch_bars_multi(polygon_key, symbols, days, timespan)

    all_data = []
    for symbol, bars in raw.items():
        if not bars:
            continue
        df = pd.DataFrame(bars)
        df = df.rename(columns={
            "open": "Open", "high": "High", "low": "Low", "close": "Close",
            "volume": "Volume",
        })
        df.index = pd.to_datetime(df["timestamp"], unit="ms")
        df["symbol"] = symbol
        if "vwap" not in df.columns or df["vwap"].isna().all():
            df["vwap"] = (df["High"] + df["Low"] + df["Close"]) / 3
        all_data.append(df[["Open", "High", "Low", "Close", "Volume", "vwap", "symbol"]])
        logger.info(f"  {symbol}: {len(df)} bars")

    if not all_data:
        raise ValueError("No data fetched for any symbol")

    combined = pd.concat(all_data, axis=0)
    combined.columns = [c.lower() for c in combined.columns]
    logger.info(f"Total bars: {len(combined)}")
    return combined


def fetch_bars_from_db(
    db_path: str,
    timespan: str = "day",
    symbols: Optional[List[str]] = None,
) -> pd.DataFrame:
    """Load OHLCV bars from the training_bars table (populated by data-loader --bars).

    Returns a DataFrame with lowercase columns: open, high, low, close, volume, vwap, symbol.
    """
    if not _HAS_SQLITE:
        raise ImportError("sqlite3 module not available")

    conn = sqlite3.connect(db_path)

    if symbols:
        placeholders = ",".join("?" for _ in symbols)
        query = f"SELECT symbol, timestamp_ms, open, high, low, close, volume, vwap FROM training_bars WHERE timespan = ? AND symbol IN ({placeholders}) ORDER BY symbol, timestamp_ms"
        params = [timespan] + symbols
    else:
        query = "SELECT symbol, timestamp_ms, open, high, low, close, volume, vwap FROM training_bars WHERE timespan = ? ORDER BY symbol, timestamp_ms"
        params = [timespan]

    df = pd.read_sql_query(query, conn, params=params)
    conn.close()

    if df.empty:
        raise ValueError(f"No bars found in training_bars (timespan={timespan})")

    df.index = pd.to_datetime(df["timestamp_ms"], unit="ms")
    df = df.drop(columns=["timestamp_ms"])

    # Fill missing vwap
    mask = df["vwap"].isna() | (df["vwap"] <= 0)
    if mask.any():
        df.loc[mask, "vwap"] = (df.loc[mask, "high"] + df.loc[mask, "low"] + df.loc[mask, "close"]) / 3

    logger.info(f"Loaded {len(df)} bars from DB ({df['symbol'].nunique()} symbols, timespan={timespan})")
    return df


def fetch_market_data(
    symbols: List[str],
    start_date: str,
    end_date: str,
    interval: str = "15m",
    use_polygon: bool = False,
) -> pd.DataFrame:
    """Fetch market data from Polygon (preferred) or yfinance (fallback).

    Uses Rust concurrent fetcher for Polygon when available (~20-50x faster).
    """
    polygon_key = os.environ.get("POLYGON_API_KEY", "")
    source = "polygon" if (use_polygon and polygon_key) else "yfinance"
    if use_polygon and not polygon_key:
        logger.warning("POLYGON_API_KEY not set, falling back to yfinance")

    # Use Rust fetcher for Polygon if available
    if source == "polygon" and _USE_RUST_FETCHER:
        start_dt = datetime.strptime(start_date, "%Y-%m-%d")
        end_dt = datetime.strptime(end_date, "%Y-%m-%d")
        days = (end_dt - start_dt).days
        return _fetch_bars_via_rust(symbols, days, interval)

    logger.info(f"Fetching data for {len(symbols)} symbols via {source} "
                f"({start_date} to {end_date}, interval={interval})")

    all_data = []
    for symbol in symbols:
        try:
            if source == "polygon":
                df = _fetch_polygon_bars(symbol, start_date, end_date, interval, polygon_key)
            else:
                df = _fetch_yfinance_bars(symbol, start_date, end_date, interval)

            if len(df) > 0:
                all_data.append(df)
                logger.info(f"  {symbol}: {len(df)} bars")
            else:
                logger.warning(f"  {symbol}: no data returned")
        except Exception as e:
            logger.error(f"  {symbol}: Error - {e}")

    if not all_data:
        raise ValueError("No data fetched for any symbol")

    combined = pd.concat(all_data, axis=0)
    # Normalize column names to lowercase for consistency across sources
    combined.columns = [c.lower() for c in combined.columns]
    logger.info(f"Total bars: {len(combined)}")

    return combined


def filter_market_data(df: pd.DataFrame) -> pd.DataFrame:
    """Filter and clean raw market data before training.

    Removes:
      - Rows with NaN/inf in OHLCV
      - Zero or negative prices
      - Zero volume bars (no trading activity)
      - Extreme outliers (>10 std from rolling mean per symbol)
      - Duplicate timestamps per symbol
      - Symbols with too few bars (<100) to be useful
    """
    initial_len = len(df)

    # Drop NaN and inf values in core columns
    core_cols = [c for c in ["open", "high", "low", "close", "volume"] if c in df.columns]
    df = df.replace([np.inf, -np.inf], np.nan)
    df = df.dropna(subset=core_cols)

    # Remove zero/negative prices
    for col in ["open", "high", "low", "close"]:
        if col in df.columns:
            df = df[df[col] > 0]

    # Remove zero volume bars
    if "volume" in df.columns:
        df = df[df["volume"] > 0]

    # Remove duplicate timestamps per symbol
    if "symbol" in df.columns:
        idx_name = df.index.name or "timestamp"
        if idx_name in df.columns:
            df = df[~df.duplicated(subset=["symbol", idx_name], keep="first")]
        else:
            # Index is the timestamp — reset, dedup, then restore
            df = df.reset_index()
            df = df[~df.duplicated(subset=["symbol", idx_name], keep="first")]
            df = df.set_index(idx_name)

    # Remove extreme price outliers per symbol (>10 std from 50-bar rolling mean)
    if "symbol" in df.columns and "close" in df.columns:
        clean_parts = []
        for symbol, group in df.groupby("symbol"):
            if len(group) < 100:
                logger.debug(f"  Skipping {symbol}: only {len(group)} bars")
                continue
            rolling_mean = group["close"].rolling(50, min_periods=10).mean()
            rolling_std = group["close"].rolling(50, min_periods=10).std()
            upper = rolling_mean + 10 * rolling_std
            lower = rolling_mean - 10 * rolling_std
            mask = (group["close"] <= upper) & (group["close"] >= lower)
            clean_parts.append(group[mask])
        if clean_parts:
            df = pd.concat(clean_parts)

    # Fill any remaining NaN in vwap with (high+low+close)/3
    if "vwap" in df.columns:
        mask = df["vwap"].isna() | (df["vwap"] <= 0)
        df.loc[mask, "vwap"] = (df.loc[mask, "high"] + df.loc[mask, "low"] + df.loc[mask, "close"]) / 3

    removed = initial_len - len(df)
    symbols_remaining = df["symbol"].nunique() if "symbol" in df.columns else "N/A"
    logger.info(f"Data filtering: {initial_len} → {len(df)} bars "
                f"({removed} removed, {symbols_remaining} symbols remaining)")

    return df


def prepare_training_data(
    df: pd.DataFrame,
    features: List[str]
) -> Tuple[np.ndarray, Dict]:
    """Prepare and normalize training data."""
    # Extract features
    data = df[features].values.copy()

    # Final safety: clip any remaining extreme values
    for i in range(data.shape[1]):
        p1, p99 = np.percentile(data[:, i], [1, 99])
        data[:, i] = np.clip(data[:, i], p1, p99)

    # Compute normalization statistics
    norm_stats = {}
    for i, feature in enumerate(features):
        mean = np.mean(data[:, i])
        std = np.std(data[:, i])
        norm_stats[feature] = {'mean': float(mean), 'std': float(std)}

        # Normalize
        data[:, i] = (data[:, i] - mean) / (std + 1e-8)

    logger.info("Data normalized")
    return data, norm_stats


def main():
    parser = argparse.ArgumentParser(description="Train PatchTST price predictor")
    parser.add_argument("--symbols", type=str, nargs="+", default=None,
                       help="Stock symbols to train on (default: all active tickers from Polygon)")
    parser.add_argument("--days", type=int, default=365, help="Number of days of historical data")
    parser.add_argument("--interval", type=str, default="15m", choices=["1m", "5m", "15m", "1h"],
                       help="Data interval")
    parser.add_argument("--epochs", type=int, default=100, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=64, help="Batch size")
    parser.add_argument("--learning-rate", type=float, default=1e-4, help="Learning rate")
    parser.add_argument("--output-dir", type=str, default="./models/price_predictor/trained",
                       help="Output directory")
    parser.add_argument("--early-stopping", type=int, default=15, help="Early stopping patience")
    parser.add_argument("--use-polygon", action="store_true",
                       help="Use Polygon.io for market data (requires POLYGON_API_KEY env var)")
    parser.add_argument("--from-db", action="store_true",
                       help="Load bars from training_bars table (populated by data-loader --bars)")
    parser.add_argument("--db-path", type=str, default="../portfolio.db", help="Path to portfolio.db")

    args = parser.parse_args()

    # Auto-enable polygon if key is available and not explicitly using yfinance
    use_polygon = args.use_polygon
    if not use_polygon and not args.from_db and os.environ.get("POLYGON_API_KEY"):
        logger.info("POLYGON_API_KEY found, auto-enabling Polygon data source")
        use_polygon = True

    # Resolve symbols: explicit list > dynamic fetch from Polygon > not needed for --from-db
    symbols = None
    if args.symbols:
        symbols = args.symbols
    elif not args.from_db:
        api_key = os.environ.get("POLYGON_API_KEY", "")
        if api_key:
            logger.info("Fetching all active tickers from Polygon...")
            symbols = fetch_active_tickers(api_key=api_key)
        else:
            logger.error("No --symbols provided and no POLYGON_API_KEY for dynamic fetch")
            sys.exit(1)

    if symbols:
        logger.info(f"Training on {len(symbols)} symbols, {args.days}d lookback")

    # Setup
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    if torch.cuda.is_available():
        device = "cuda"
    elif torch.backends.mps.is_available():
        device = "mps"
    else:
        device = "cpu"
    logger.info(f"Using device: {device}")

    # Fetch data — prefer DB if --from-db or if DB has sufficient data
    db_path = Path(args.db_path)
    if args.from_db or (not use_polygon and db_path.exists()):
        try:
            df = fetch_bars_from_db(str(db_path), symbols=symbols)
            if len(df) < 1000 and not args.from_db:
                logger.info(f"DB has only {len(df)} bars, falling back to Polygon")
                raise ValueError("insufficient DB data")
        except Exception as e:
            if args.from_db:
                logger.error(f"--from-db specified but DB load failed: {e}")
                sys.exit(1)
            logger.info(f"DB load failed ({e}), falling back to Polygon fetch")
            end_date = datetime.now()
            start_date = end_date - timedelta(days=args.days)
            df = fetch_market_data(
                symbols=symbols,
                start_date=start_date.strftime("%Y-%m-%d"),
                end_date=end_date.strftime("%Y-%m-%d"),
                interval=args.interval,
                use_polygon=use_polygon,
            )
    else:
        end_date = datetime.now()
        start_date = end_date - timedelta(days=args.days)
        df = fetch_market_data(
            symbols=symbols,
            start_date=start_date.strftime("%Y-%m-%d"),
            end_date=end_date.strftime("%Y-%m-%d"),
            interval=args.interval,
            use_polygon=use_polygon,
        )

    # Filter bad data
    df = filter_market_data(df)

    if len(df) == 0:
        logger.error("No data remaining after filtering")
        sys.exit(1)

    # Prepare data
    features = config.price_predictor.features
    data, norm_stats = prepare_training_data(df, features)

    # Save normalization stats
    with open(output_dir / "normalization_stats.json", "w") as f:
        json.dump(norm_stats, f, indent=2)

    # Split data
    train_size = int(0.8 * len(data))
    train_data = data[:train_size]
    val_data = data[train_size:]

    logger.info(f"Train samples: {len(train_data)}")
    logger.info(f"Val samples: {len(val_data)}")

    # Create datasets
    train_dataset = TimeSeriesDataset(
        train_data,
        context_length=config.price_predictor.context_length,
        prediction_length=config.price_predictor.prediction_length,
        stride=4
    )
    val_dataset = TimeSeriesDataset(
        val_data,
        context_length=config.price_predictor.context_length,
        prediction_length=config.price_predictor.prediction_length,
        stride=4
    )

    # num_workers>0 with fork crashes on macOS (ObjC fork safety),
    # but 'spawn' context is safe. pin_memory only benefits CUDA.
    import platform
    use_cuda = device == "cuda"
    if use_cuda:
        num_workers = 4
        mp_context = None
    elif platform.system() == "Darwin" and device == "mps":
        num_workers = 4
        mp_context = "spawn"
    else:
        num_workers = 0
        mp_context = None

    loader_kwargs = dict(
        batch_size=args.batch_size,
        num_workers=num_workers,
        pin_memory=use_cuda,
        persistent_workers=num_workers > 0,
        multiprocessing_context=mp_context if num_workers > 0 else None,
    )

    train_loader = DataLoader(
        train_dataset,
        shuffle=True,
        **loader_kwargs,
    )
    val_loader = DataLoader(
        val_dataset,
        shuffle=False,
        **loader_kwargs,
    )

    logger.info(f"Train batches: {len(train_loader)}")
    logger.info(f"Val batches: {len(val_loader)}")

    # Create model
    model_config = {
        'context_length': config.price_predictor.context_length,
        'prediction_length': config.price_predictor.prediction_length,
        'num_features': len(features),
        'patch_size': config.price_predictor.patch_size,
        'num_layers': config.price_predictor.num_layers,
        'd_model': config.price_predictor.d_model,
        'num_heads': config.price_predictor.num_heads,
        'dim_feedforward': config.price_predictor.d_model * 4,
        'dropout': config.price_predictor.dropout,
        'features': features
    }

    model = create_model(model_config)
    logger.info(f"Model parameters: {sum(p.numel() for p in model.parameters()):,}")

    # Save config
    with open(output_dir / "config.json", "w") as f:
        json.dump(model_config, f, indent=2)

    # Compile model for faster training (PyTorch 2.0+, CUDA only — inductor
    # backend does not support MPS and adds overhead without benefit)
    if hasattr(torch, 'compile') and device == "cuda":
        logger.info("Compiling model with torch.compile")
        try:
            model = torch.compile(model)
        except Exception as e:
            logger.warning(f"Failed to compile model: {e}")

    # Create trainer
    trainer = PatchTSTTrainer(
        model=model,
        device=device,
        learning_rate=args.learning_rate
    )

    # Training loop
    best_val_loss = float('inf')
    patience_counter = 0

    for epoch in range(args.epochs):
        logger.info(f"\nEpoch {epoch + 1}/{args.epochs}")

        # Train
        train_loss, train_price_loss, train_dir_loss = trainer.train_epoch(train_loader)

        # Validate
        val_loss, val_price_loss, val_dir_loss, val_accuracy = trainer.validate(val_loader)

        logger.info(f"Train - Loss: {train_loss:.4f}, Price: {train_price_loss:.4f}, Dir: {train_dir_loss:.4f}")
        logger.info(f"Val   - Loss: {val_loss:.4f}, Price: {val_price_loss:.4f}, Dir: {val_dir_loss:.4f}, Acc: {val_accuracy:.4f}")

        # Learning rate scheduling
        trainer.scheduler.step(val_loss)

        # Save best model
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            patience_counter = 0

            metrics = {
                'val_loss': val_loss,
                'val_price_loss': val_price_loss,
                'val_direction_loss': val_dir_loss,
                'val_accuracy': val_accuracy
            }

            trainer.save_checkpoint(
                output_dir / "model.pt",
                epoch,
                val_loss,
                metrics
            )
            logger.info(f"New best model! Val loss: {val_loss:.4f}")
        else:
            patience_counter += 1

        # Early stopping
        if patience_counter >= args.early_stopping:
            logger.info(f"Early stopping after {epoch + 1} epochs")
            break

    # Save to database
    db = MLDatabase(config.database_path)
    db.save_model_metadata(
        model_name="patchtst_price_predictor",
        model_type="price_prediction",
        version="v1",
        path=str(output_dir),
        metrics={
            'best_val_loss': best_val_loss,
            'best_val_accuracy': val_accuracy
        },
        config=model_config
    )

    logger.info(f"\nTraining complete! Best val loss: {best_val_loss:.4f}")
    logger.info(f"Model saved to: {output_dir}")


if __name__ == "__main__":
    main()
