"""PatchTST Training Script."""
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
import yfinance as yf
from datetime import datetime, timedelta
from typing import List, Tuple, Optional

sys.path.append(str(Path(__file__).parent.parent))
from shared.config import config
from shared.database import MLDatabase
from price_predictor.model import PatchTST, create_model


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
            verbose=True
        )

        # Loss functions
        self.price_criterion = nn.MSELoss()
        self.direction_criterion = nn.CrossEntropyLoss()

        self.best_val_loss = float('inf')

    def train_epoch(self, train_loader: DataLoader) -> Tuple[float, float, float]:
        """Train for one epoch."""
        self.model.train()

        total_loss = 0
        total_price_loss = 0
        total_direction_loss = 0
        num_batches = 0

        pbar = tqdm(train_loader, desc="Training")
        for x, y_price, y_direction in pbar:
            x = x.to(self.device)
            y_price = y_price.to(self.device)
            y_direction = y_direction.to(self.device)

            # Forward pass
            price_pred, direction_logits = self.model(x)

            # Compute losses
            price_loss = self.price_criterion(price_pred, y_price)

            direction_logits_flat = direction_logits.reshape(-1, 3)
            y_direction_flat = y_direction.reshape(-1)
            direction_loss = self.direction_criterion(direction_logits_flat, y_direction_flat)

            # Combined loss (weighted)
            loss = price_loss + 0.5 * direction_loss

            # Backward pass
            self.optimizer.zero_grad()
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
            x = x.to(self.device)
            y_price = y_price.to(self.device)
            y_direction = y_direction.to(self.device)

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


def fetch_market_data(
    symbols: List[str],
    start_date: str,
    end_date: str,
    interval: str = "15m"
) -> pd.DataFrame:
    """Fetch market data from yfinance."""
    logger.info(f"Fetching data for {len(symbols)} symbols from {start_date} to {end_date}")

    all_data = []
    for symbol in symbols:
        try:
            ticker = yf.Ticker(symbol)
            df = ticker.history(start=start_date, end=end_date, interval=interval)

            if len(df) > 0:
                df['symbol'] = symbol
                df['vwap'] = ((df['High'] + df['Low'] + df['Close']) / 3)
                all_data.append(df)
                logger.info(f"  {symbol}: {len(df)} bars")
        except Exception as e:
            logger.error(f"  {symbol}: Error - {e}")

    if not all_data:
        raise ValueError("No data fetched")

    combined = pd.concat(all_data, axis=0)
    logger.info(f"Total bars: {len(combined)}")

    return combined


def prepare_training_data(
    df: pd.DataFrame,
    features: List[str]
) -> Tuple[np.ndarray, Dict]:
    """Prepare and normalize training data."""
    # Extract features
    data = df[features].values

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
    parser.add_argument("--symbols", type=str, nargs="+", default=["SPY", "QQQ", "AAPL", "MSFT", "GOOGL"],
                       help="Stock symbols to train on")
    parser.add_argument("--days", type=int, default=60, help="Number of days of historical data")
    parser.add_argument("--interval", type=str, default="15m", choices=["1m", "5m", "15m", "1h"],
                       help="Data interval")
    parser.add_argument("--epochs", type=int, default=50, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=64, help="Batch size")
    parser.add_argument("--learning-rate", type=float, default=1e-4, help="Learning rate")
    parser.add_argument("--output-dir", type=str, default="./models/price_predictor/trained",
                       help="Output directory")
    parser.add_argument("--early-stopping", type=int, default=10, help="Early stopping patience")

    args = parser.parse_args()

    # Setup
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    device = "cuda" if torch.cuda.is_available() else "cpu"
    logger.info(f"Using device: {device}")

    # Fetch data
    end_date = datetime.now()
    start_date = end_date - timedelta(days=args.days)

    df = fetch_market_data(
        symbols=args.symbols,
        start_date=start_date.strftime("%Y-%m-%d"),
        end_date=end_date.strftime("%Y-%m-%d"),
        interval=args.interval
    )

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

    train_loader = DataLoader(
        train_dataset,
        batch_size=args.batch_size,
        shuffle=True,
        num_workers=4,
        pin_memory=True
    )
    val_loader = DataLoader(
        val_dataset,
        batch_size=args.batch_size,
        shuffle=False,
        num_workers=4,
        pin_memory=True
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
