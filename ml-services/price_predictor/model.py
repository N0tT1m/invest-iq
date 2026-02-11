"""PatchTST Price Direction Predictor."""
import torch
import torch.nn as nn
import torch.nn.functional as F
import numpy as np
from typing import Dict, List, Tuple, Optional
from pathlib import Path
from loguru import logger
import json


class PatchEmbedding(nn.Module):
    """Patch embedding layer for time series."""

    def __init__(self, patch_size: int, d_model: int, num_features: int):
        super().__init__()
        self.patch_size = patch_size
        self.d_model = d_model

        # Linear projection of flattened patches
        self.linear = nn.Linear(patch_size * num_features, d_model)

    def forward(self, x):
        """
        Args:
            x: (batch_size, seq_len, num_features)
        Returns:
            patches: (batch_size, num_patches, d_model)
        """
        batch_size, seq_len, num_features = x.shape

        # Create patches
        num_patches = seq_len // self.patch_size
        x = x[:, :num_patches * self.patch_size, :]  # Truncate to fit patches

        # Reshape into patches
        x = x.reshape(batch_size, num_patches, self.patch_size * num_features)

        # Linear projection
        x = self.linear(x)

        return x


class TransformerEncoderLayer(nn.Module):
    """Transformer encoder layer."""

    def __init__(self, d_model: int, num_heads: int, dim_feedforward: int, dropout: float):
        super().__init__()
        self.self_attn = nn.MultiheadAttention(d_model, num_heads, dropout=dropout, batch_first=True)
        self.linear1 = nn.Linear(d_model, dim_feedforward)
        self.dropout = nn.Dropout(dropout)
        self.linear2 = nn.Linear(dim_feedforward, d_model)

        self.norm1 = nn.LayerNorm(d_model)
        self.norm2 = nn.LayerNorm(d_model)
        self.dropout1 = nn.Dropout(dropout)
        self.dropout2 = nn.Dropout(dropout)

    def forward(self, x):
        # Self attention
        x2 = self.norm1(x)
        x2, _ = self.self_attn(x2, x2, x2)
        x = x + self.dropout1(x2)

        # Feedforward
        x2 = self.norm2(x)
        x2 = self.linear2(self.dropout(F.relu(self.linear1(x2))))
        x = x + self.dropout2(x2)

        return x


class PatchTST(nn.Module):
    """
    PatchTST: A Time Series Transformer Using Patching.

    Reference: https://arxiv.org/abs/2211.14730
    """

    def __init__(
        self,
        context_length: int,
        prediction_length: int,
        num_features: int,
        patch_size: int = 16,
        num_layers: int = 3,
        d_model: int = 128,
        num_heads: int = 4,
        dim_feedforward: int = 512,
        dropout: float = 0.1
    ):
        super().__init__()

        self.context_length = context_length
        self.prediction_length = prediction_length
        self.num_features = num_features
        self.patch_size = patch_size

        # Patch embedding
        self.patch_embedding = PatchEmbedding(patch_size, d_model, num_features)

        # Positional encoding
        num_patches = context_length // patch_size
        self.pos_embedding = nn.Parameter(torch.randn(1, num_patches, d_model))

        # Transformer encoder
        self.encoder_layers = nn.ModuleList([
            TransformerEncoderLayer(d_model, num_heads, dim_feedforward, dropout)
            for _ in range(num_layers)
        ])

        # Output heads
        self.flatten_head = nn.Linear(d_model * num_patches, prediction_length)
        self.direction_head = nn.Linear(d_model * num_patches, prediction_length * 3)  # 3 classes: up/down/neutral

        self.dropout = nn.Dropout(dropout)

    def forward(self, x):
        """
        Args:
            x: (batch_size, context_length, num_features)

        Returns:
            price_pred: (batch_size, prediction_length) - predicted prices
            direction_logits: (batch_size, prediction_length, 3) - direction logits
        """
        # Patch embedding
        x = self.patch_embedding(x)  # (batch_size, num_patches, d_model)

        # Add positional encoding
        x = x + self.pos_embedding

        # Transformer encoder
        for layer in self.encoder_layers:
            x = layer(x)

        # Flatten
        x = x.flatten(start_dim=1)  # (batch_size, num_patches * d_model)
        x = self.dropout(x)

        # Price prediction
        price_pred = self.flatten_head(x)  # (batch_size, prediction_length)

        # Direction classification
        direction_logits = self.direction_head(x)  # (batch_size, prediction_length * 3)
        direction_logits = direction_logits.reshape(-1, self.prediction_length, 3)

        return price_pred, direction_logits


class PricePredictorInference:
    """Inference wrapper for PatchTST price predictor."""

    def __init__(
        self,
        model_path: str,
        device: str = "cuda",
        compile: bool = True
    ):
        if torch.cuda.is_available():
            resolved_device = "cuda"
        elif torch.backends.mps.is_available():
            resolved_device = "mps"
        else:
            resolved_device = "cpu"
        self._device_type = resolved_device
        self.device = torch.device(resolved_device)
        self.model_path = Path(model_path)
        self.compile = compile

        # Load config
        config_path = self.model_path / "config.json"
        if config_path.exists():
            with open(config_path) as f:
                self.config = json.load(f)
        else:
            raise FileNotFoundError(f"Config not found at {config_path}")

        # Load model
        self.model = self._load_model()
        self.model.eval()

        # Load normalization stats
        stats_path = self.model_path / "normalization_stats.json"
        if stats_path.exists():
            with open(stats_path) as f:
                self.norm_stats = json.load(f)
        else:
            logger.warning("Normalization stats not found, using defaults")
            self.norm_stats = None

        logger.info(f"Price predictor loaded from {model_path}")

    def _load_model(self) -> PatchTST:
        """Load model from checkpoint."""
        model = PatchTST(
            context_length=self.config['context_length'],
            prediction_length=self.config['prediction_length'],
            num_features=self.config['num_features'],
            patch_size=self.config['patch_size'],
            num_layers=self.config['num_layers'],
            d_model=self.config['d_model'],
            num_heads=self.config['num_heads'],
            dim_feedforward=self.config.get('dim_feedforward', 512),
            dropout=self.config['dropout']
        )

        # Load weights
        checkpoint_path = self.model_path / "model.pt"
        if checkpoint_path.exists():
            checkpoint = torch.load(checkpoint_path, map_location=self.device)
            model.load_state_dict(checkpoint['model_state_dict'])
            logger.info(f"Loaded checkpoint from epoch {checkpoint.get('epoch', 'unknown')}")
        else:
            logger.warning("No checkpoint found, using random weights")

        model = model.to(self.device)

        # Compile for faster inference (CUDA only — MPS not supported)
        if self.compile and hasattr(torch, 'compile') and self._device_type == "cuda":
            try:
                model = torch.compile(model)
                logger.info("Model compiled with torch.compile")
            except Exception as e:
                logger.warning(f"Failed to compile model: {e}")

        return model

    def normalize(self, data: np.ndarray) -> np.ndarray:
        """Normalize input data."""
        if self.norm_stats is None:
            return data

        data = data.copy()
        for i, feature in enumerate(self.config.get('features', [])):
            if feature in self.norm_stats:
                mean = self.norm_stats[feature]['mean']
                std = self.norm_stats[feature]['std']
                data[:, :, i] = (data[:, :, i] - mean) / (std + 1e-8)

        return data

    def denormalize_price(self, prices: np.ndarray, feature: str = 'close') -> np.ndarray:
        """Denormalize predicted prices."""
        if self.norm_stats is None or feature not in self.norm_stats:
            return prices

        mean = self.norm_stats[feature]['mean']
        std = self.norm_stats[feature]['std']
        return prices * std + mean

    @torch.no_grad()
    def predict(
        self,
        history: np.ndarray,
        return_probabilities: bool = True
    ) -> Dict:
        """
        Predict future price direction and values.

        Args:
            history: (batch_size, context_length, num_features) or (context_length, num_features)
            return_probabilities: Return class probabilities

        Returns:
            Dictionary with predictions
        """
        # Handle single sample
        if history.ndim == 2:
            history = history[np.newaxis, ...]

        # Normalize
        history = self.normalize(history)

        # Convert to tensor
        x = torch.from_numpy(history).float().to(self.device)

        # Forward pass
        price_pred, direction_logits = self.model(x)

        # Convert to numpy
        price_pred = price_pred.cpu().numpy()
        direction_logits = direction_logits.cpu().numpy()

        # Denormalize prices
        price_pred = self.denormalize_price(price_pred)

        # Get direction predictions
        direction_probs = self._softmax(direction_logits, axis=-1)
        direction_classes = np.argmax(direction_probs, axis=-1)

        # Map to labels: 0=up, 1=down, 2=neutral
        label_map = {0: 'up', 1: 'down', 2: 'neutral'}

        results = []
        for i in range(len(history)):
            result = {
                'predicted_prices': price_pred[i].tolist(),
                'predicted_directions': [label_map[c] for c in direction_classes[i]],
                'direction_probabilities': {
                    'up': direction_probs[i, :, 0].tolist(),
                    'down': direction_probs[i, :, 1].tolist(),
                    'neutral': direction_probs[i, :, 2].tolist()
                } if return_probabilities else None,
                'confidence': np.max(direction_probs[i], axis=-1).tolist(),
                'prediction_horizon': self.config['prediction_length']
            }
            results.append(result)

        return results[0] if len(results) == 1 else results

    def predict_next_direction(
        self,
        history: np.ndarray,
        horizon_steps: int = 1
    ) -> Dict:
        """
        Predict direction for next N steps with aggregated confidence.

        Args:
            history: Historical price data
            horizon_steps: Number of future steps to predict (default: 1 for next step)

        Returns:
            Aggregated prediction with confidence
        """
        predictions = self.predict(history, return_probabilities=True)

        # Take first horizon_steps predictions
        directions = predictions['predicted_directions'][:horizon_steps]
        up_probs = predictions['direction_probabilities']['up'][:horizon_steps]
        down_probs = predictions['direction_probabilities']['down'][:horizon_steps]
        neutral_probs = predictions['direction_probabilities']['neutral'][:horizon_steps]

        # Aggregate probabilities
        avg_up = np.mean(up_probs)
        avg_down = np.mean(down_probs)
        avg_neutral = np.mean(neutral_probs)

        # Determine overall direction
        if avg_up > avg_down and avg_up > avg_neutral:
            direction = 'up'
            confidence = avg_up
        elif avg_down > avg_up and avg_down > avg_neutral:
            direction = 'down'
            confidence = avg_down
        else:
            direction = 'neutral'
            confidence = avg_neutral

        return {
            'direction': direction,
            'confidence': float(confidence),
            'probabilities': {
                'up': float(avg_up),
                'down': float(avg_down),
                'neutral': float(avg_neutral)
            },
            'horizon_steps': horizon_steps,
            'predicted_prices': predictions['predicted_prices'][:horizon_steps]
        }

    @staticmethod
    def _softmax(x, axis=-1):
        """Compute softmax."""
        exp_x = np.exp(x - np.max(x, axis=axis, keepdims=True))
        return exp_x / np.sum(exp_x, axis=axis, keepdims=True)


def create_model(config: Dict) -> PatchTST:
    """Factory function to create PatchTST model."""
    return PatchTST(
        context_length=config['context_length'],
        prediction_length=config['prediction_length'],
        num_features=config['num_features'],
        patch_size=config['patch_size'],
        num_layers=config['num_layers'],
        d_model=config['d_model'],
        num_heads=config['num_heads'],
        dim_feedforward=config.get('dim_feedforward', 512),
        dropout=config['dropout']
    )
