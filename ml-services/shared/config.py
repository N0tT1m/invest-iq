"""Configuration management for ML services."""
import yaml
from pathlib import Path
from typing import Dict, Any
from pydantic import BaseModel, Field
from pydantic_settings import BaseSettings


class SentimentConfig(BaseModel):
    model_name: str = "ProsusAI/finbert"
    max_length: int = 512
    batch_size: int = 32
    cache_dir: str = "./models/sentiment"
    quantize: bool = False


class BayesianConfig(BaseModel):
    prior_alpha: float = 1.0
    prior_beta: float = 1.0
    decay_factor: float = 0.95
    min_samples: int = 10
    exploration_rate: float = 0.1


class PricePredictorConfig(BaseModel):
    model_name: str = "patchtst"
    context_length: int = 512
    prediction_length: int = 12
    patch_size: int = 16
    num_layers: int = 3
    d_model: int = 128
    num_heads: int = 4
    dropout: float = 0.1
    lookback_hours: int = 168
    features: list[str] = Field(default_factory=lambda: ["open", "high", "low", "close", "volume", "vwap"])
    cache_dir: str = "./models/price_predictor"


class SignalModelsConfig(BaseModel):
    model_dir: str = "./models/signal_models"
    meta_model_threshold: float = 0.6
    min_training_samples: int = 100
    retrain_interval_days: int = 7


class GPUConfig(BaseModel):
    device: str = "cuda"
    mixed_precision: bool = True
    compile: bool = True


class ServiceConfig(BaseModel):
    host: str = "0.0.0.0"
    port_sentiment: int = 8001
    port_bayesian: int = 8002
    port_price_predictor: int = 8003
    port_signal_models: int = 8004
    workers: int = 1
    log_level: str = "info"


class InferenceConfig(BaseModel):
    max_batch_size: int = 32
    timeout_seconds: int = 5
    cache_predictions: bool = True
    cache_ttl_seconds: int = 60


class Config(BaseSettings):
    """Main configuration class."""
    service: ServiceConfig = Field(default_factory=ServiceConfig)
    gpu: GPUConfig = Field(default_factory=GPUConfig)
    sentiment: SentimentConfig = Field(default_factory=SentimentConfig)
    bayesian: BayesianConfig = Field(default_factory=BayesianConfig)
    price_predictor: PricePredictorConfig = Field(default_factory=PricePredictorConfig)
    signal_models: SignalModelsConfig = Field(default_factory=SignalModelsConfig)
    inference: InferenceConfig = Field(default_factory=InferenceConfig)
    database_path: str = "../portfolio.db"

    @classmethod
    def load(cls, config_path: str = "config.yaml") -> "Config":
        """Load configuration from YAML file."""
        path = Path(config_path)
        if not path.exists():
            return cls()

        with open(path) as f:
            data = yaml.safe_load(f)

        # Flatten nested structure
        flat_data = {}
        if "service" in data:
            flat_data["service"] = ServiceConfig(**data["service"])
        if "gpu" in data:
            flat_data["gpu"] = GPUConfig(**data["gpu"])
        if "models" in data:
            if "sentiment" in data["models"]:
                flat_data["sentiment"] = SentimentConfig(**data["models"]["sentiment"])
            if "bayesian" in data["models"]:
                flat_data["bayesian"] = BayesianConfig(**data["models"]["bayesian"])
            if "price_predictor" in data["models"]:
                flat_data["price_predictor"] = PricePredictorConfig(**data["models"]["price_predictor"])
            if "signal_models" in data["models"]:
                flat_data["signal_models"] = SignalModelsConfig(**data["models"]["signal_models"])
        if "inference" in data:
            flat_data["inference"] = InferenceConfig(**data["inference"])
        if "database" in data and "path" in data["database"]:
            flat_data["database_path"] = data["database"]["path"]

        return cls(**flat_data)


# Global config instance
config = Config.load()
