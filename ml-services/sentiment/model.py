"""FinBERT Sentiment Analysis Model."""
import torch
import torch.nn as nn
import transformers
from transformers import AutoTokenizer, AutoModelForSequenceClassification
from typing import List, Dict, Tuple
import numpy as np
from pathlib import Path
from loguru import logger


def _load_model_quiet(cls, *args, **kwargs):
    """Load a transformers model with noisy warnings suppressed.

    Newer transformers removed position_ids from BERT's buffers, but older
    checkpoints (like ProsusAI/finbert) still ship it.  The resulting
    "UNEXPECTED" key warning is harmless — silence it here.
    """
    prev = transformers.logging.get_verbosity()
    transformers.logging.set_verbosity_error()
    try:
        return cls.from_pretrained(*args, **kwargs)
    finally:
        transformers.logging.set_verbosity(prev)


class FinBERTSentiment:
    """FinBERT model for financial sentiment analysis."""

    def __init__(
        self,
        model_name: str = "ProsusAI/finbert",
        device: str = "cuda",
        quantize: bool = False,
        compile: bool = True,
        cache_dir: str = "./models/sentiment"
    ):
        if torch.cuda.is_available():
            resolved_device = "cuda"
        elif torch.backends.mps.is_available():
            resolved_device = "mps"
        else:
            resolved_device = "cpu"
        self.device = torch.device(resolved_device)
        self.model_name = model_name
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        logger.info(f"Loading FinBERT model: {model_name}")
        logger.info(f"Device: {self.device}")

        # Load tokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(
            model_name,
            cache_dir=str(self.cache_dir)
        )

        # Load model (suppress position_ids UNEXPECTED warning from older checkpoints)
        self.model = _load_model_quiet(
            AutoModelForSequenceClassification,
            model_name,
            cache_dir=str(self.cache_dir)
        )

        # Quantization for faster inference
        if quantize and device == "cuda":
            logger.info("Applying INT8 quantization")
            from transformers import BitsAndBytesConfig
            quantization_config = BitsAndBytesConfig(
                load_in_8bit=True,
                llm_int8_threshold=6.0
            )
            self.model = _load_model_quiet(
                AutoModelForSequenceClassification,
                model_name,
                cache_dir=str(self.cache_dir),
                quantization_config=quantization_config,
                device_map="auto"
            )
        else:
            self.model = self.model.to(self.device)

        self.model.eval()

        # Compile model for faster inference (PyTorch 2.0+, CUDA and MPS supported)
        if compile and hasattr(torch, 'compile') and resolved_device in ("cuda", "mps"):
            logger.info(f"Compiling model with torch.compile (device={resolved_device})")
            try:
                self.model = torch.compile(self.model)
            except Exception as e:
                logger.warning(f"Failed to compile model: {e}")

        # Label mapping
        self.labels = {0: "positive", 1: "negative", 2: "neutral"}
        logger.info("FinBERT model loaded successfully")

    @torch.no_grad()
    def predict(self, texts: List[str], batch_size: int = 32) -> List[Dict[str, float]]:
        """
        Predict sentiment for a list of texts.

        Args:
            texts: List of text strings to analyze
            batch_size: Batch size for inference

        Returns:
            List of dictionaries with sentiment predictions
        """
        results = []

        for i in range(0, len(texts), batch_size):
            batch_texts = texts[i:i + batch_size]

            # Tokenize
            inputs = self.tokenizer(
                batch_texts,
                padding=True,
                truncation=True,
                max_length=512,
                return_tensors="pt"
            ).to(self.device)

            # Forward pass
            outputs = self.model(**inputs)
            logits = outputs.logits

            # Get probabilities
            probs = torch.nn.functional.softmax(logits, dim=-1)

            # Convert to numpy
            probs_np = probs.cpu().numpy()

            # Process each prediction
            for prob in probs_np:
                predicted_class = int(np.argmax(prob))
                confidence = float(prob[predicted_class])

                result = {
                    "label": self.labels[predicted_class],
                    "positive": float(prob[0]),
                    "negative": float(prob[1]),
                    "neutral": float(prob[2]),
                    "confidence": confidence,
                    "score": self._compute_sentiment_score(prob)
                }
                results.append(result)

        return results

    def _compute_sentiment_score(self, probs: np.ndarray) -> float:
        """
        Compute a sentiment score from -1 (very negative) to +1 (very positive).

        Args:
            probs: Array of [positive, negative, neutral] probabilities

        Returns:
            Sentiment score between -1 and 1
        """
        positive_prob = probs[0]
        negative_prob = probs[1]
        neutral_prob = probs[2]

        # Weight positive and negative, discount neutral
        score = positive_prob - negative_prob
        return float(score)

    def analyze_news(self, headlines: List[str], descriptions: List[str] = None) -> Dict[str, float]:
        """
        Analyze multiple news items and compute aggregate sentiment.

        Args:
            headlines: List of news headlines
            descriptions: Optional list of news descriptions

        Returns:
            Dictionary with aggregate sentiment metrics
        """
        all_texts = headlines.copy()
        if descriptions:
            all_texts.extend([d for d in descriptions if d])

        if not all_texts:
            return {
                "overall_sentiment": "neutral",
                "score": 0.0,
                "confidence": 0.0,
                "positive_ratio": 0.0,
                "negative_ratio": 0.0
            }

        # Get predictions
        predictions = self.predict(all_texts)

        # Aggregate results
        scores = [p["score"] for p in predictions]
        confidences = [p["confidence"] for p in predictions]

        positive_count = sum(1 for p in predictions if p["label"] == "positive")
        negative_count = sum(1 for p in predictions if p["label"] == "negative")
        neutral_count = sum(1 for p in predictions if p["label"] == "neutral")

        total = len(predictions)
        avg_score = np.mean(scores)
        avg_confidence = np.mean(confidences)

        # Determine overall sentiment
        if avg_score > 0.2:
            overall = "positive"
        elif avg_score < -0.2:
            overall = "negative"
        else:
            overall = "neutral"

        return {
            "overall_sentiment": overall,
            "score": float(avg_score),
            "confidence": float(avg_confidence),
            "positive_ratio": positive_count / total,
            "negative_ratio": negative_count / total,
            "neutral_ratio": neutral_count / total,
            "article_count": total,
            "detailed_predictions": predictions
        }

    def save_model(self, path: str):
        """Save fine-tuned model."""
        save_path = Path(path)
        save_path.mkdir(parents=True, exist_ok=True)

        self.model.save_pretrained(str(save_path))
        self.tokenizer.save_pretrained(str(save_path))
        logger.info(f"Model saved to {path}")

    def load_model(self, path: str):
        """Load fine-tuned model."""
        logger.info(f"Loading model from {path}")
        self.tokenizer = AutoTokenizer.from_pretrained(path)
        self.model = _load_model_quiet(AutoModelForSequenceClassification, path)
        self.model = self.model.to(self.device)
        self.model.eval()
        logger.info("Model loaded successfully")


class SentimentCache:
    """Simple in-memory cache for sentiment predictions."""

    def __init__(self, max_size: int = 1000, ttl_seconds: int = 60):
        self.cache: Dict[str, Tuple[Dict, float]] = {}
        self.max_size = max_size
        self.ttl_seconds = ttl_seconds

    def get(self, text: str) -> Dict | None:
        """Get cached prediction if available and not expired."""
        if text not in self.cache:
            return None

        prediction, timestamp = self.cache[text]
        import time
        if time.time() - timestamp > self.ttl_seconds:
            del self.cache[text]
            return None

        return prediction

    def set(self, text: str, prediction: Dict):
        """Cache a prediction."""
        import time

        # Evict oldest if cache is full
        if len(self.cache) >= self.max_size:
            oldest_key = min(self.cache.keys(), key=lambda k: self.cache[k][1])
            del self.cache[oldest_key]

        self.cache[text] = (prediction, time.time())

    def clear(self):
        """Clear cache."""
        self.cache.clear()
