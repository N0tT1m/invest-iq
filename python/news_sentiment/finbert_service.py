"""
FinBERT Sentiment Analysis Service

This service uses FinBERT (BERT fine-tuned for financial sentiment) to analyze
news articles and provide sentiment scores for trading decisions.

FinBERT is specifically trained on financial texts and outperforms general
sentiment models for finance-related content.

Run with: uvicorn finbert_service:app --host 0.0.0.0 --port 8002
"""

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import logging
from typing import Optional
import re

# Try to import transformers for FinBERT
try:
    from transformers import AutoTokenizer, AutoModelForSequenceClassification
    import torch
    TRANSFORMERS_AVAILABLE = True
except ImportError:
    TRANSFORMERS_AVAILABLE = False
    logging.warning("transformers not available. Install with: pip install transformers torch")

app = FastAPI(title="FinBERT Sentiment Analysis Service")

# Logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class SentimentRequest(BaseModel):
    text: str


class SentimentResponse(BaseModel):
    sentiment: str  # "positive", "negative", "neutral"
    score: float    # -1.0 to 1.0
    confidence: float  # 0.0 to 1.0
    reasoning: str


class FinBERTAnalyzer:
    """FinBERT-based sentiment analyzer"""

    def __init__(self):
        self.model = None
        self.tokenizer = None
        self.device = None

        if TRANSFORMERS_AVAILABLE:
            try:
                self._load_model()
            except Exception as e:
                logger.error(f"Failed to load FinBERT model: {e}")
                logger.info("Falling back to keyword-based sentiment")

    def _load_model(self):
        """Load FinBERT model from HuggingFace"""
        model_name = "ProsusAI/finbert"

        logger.info(f"Loading FinBERT model: {model_name}")

        # Check for GPU availability
        if torch.cuda.is_available():
            self.device = torch.device("cuda")
            logger.info(f"Using GPU: {torch.cuda.get_device_name(0)}")
        else:
            self.device = torch.device("cpu")
            logger.info("Using CPU (GPU not available)")

        # Load model and tokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(model_name)
        self.model = AutoModelForSequenceClassification.from_pretrained(model_name)
        self.model.to(self.device)
        self.model.eval()

        logger.info("FinBERT model loaded successfully")

    def analyze(self, text: str) -> SentimentResponse:
        """Analyze sentiment of text"""

        if self.model is not None and self.tokenizer is not None:
            return self._finbert_analyze(text)
        else:
            return self._keyword_analyze(text)

    def _finbert_analyze(self, text: str) -> SentimentResponse:
        """Analyze using FinBERT model"""

        # Preprocess text
        text = self._preprocess_text(text)

        # Tokenize
        inputs = self.tokenizer(
            text,
            return_tensors="pt",
            truncation=True,
            max_length=512,
            padding=True
        )
        inputs = {k: v.to(self.device) for k, v in inputs.items()}

        # Get predictions
        with torch.no_grad():
            outputs = self.model(**inputs)
            logits = outputs.logits
            probabilities = torch.nn.functional.softmax(logits, dim=-1)

        # FinBERT classes: 0=positive, 1=negative, 2=neutral
        probs = probabilities[0].cpu().numpy()
        positive_prob = float(probs[0])
        negative_prob = float(probs[1])
        neutral_prob = float(probs[2])

        # Determine sentiment
        max_prob = max(positive_prob, negative_prob, neutral_prob)

        if positive_prob == max_prob:
            sentiment = "positive"
            score = positive_prob
        elif negative_prob == max_prob:
            sentiment = "negative"
            score = -negative_prob
        else:
            sentiment = "neutral"
            score = 0.0

        reasoning = (
            f"FinBERT analysis: "
            f"positive={positive_prob:.2f}, "
            f"negative={negative_prob:.2f}, "
            f"neutral={neutral_prob:.2f}"
        )

        return SentimentResponse(
            sentiment=sentiment,
            score=score,
            confidence=max_prob,
            reasoning=reasoning
        )

    def _keyword_analyze(self, text: str) -> SentimentResponse:
        """Fallback keyword-based analysis"""

        text_lower = text.lower()

        positive_keywords = [
            "surges", "rally", "rallies", "gains", "profit", "growth", "beats",
            "exceeds", "strong", "bullish", "upgrade", "upgraded", "optimistic",
            "breakthrough", "success", "successful", "record", "high", "soars",
            "outperforms", "positive", "rose", "rising", "climbs", "jumped",
            "expansion", "recovered", "improved", "improvement", "beat expectations",
        ]

        negative_keywords = [
            "falls", "plunges", "plummeted", "losses", "decline", "declines",
            "weak", "weakness", "misses", "missed", "cuts", "drops", "dropped",
            "bearish", "downgrade", "downgraded", "pessimistic", "failure",
            "concern", "concerns", "warning", "warns", "low", "crashes", "crashed",
            "underperforms", "negative", "fell", "falling", "slides", "slumped",
            "contraction", "layoffs", "bankruptcy", "investigation", "lawsuit",
        ]

        positive_count = sum(1 for kw in positive_keywords if kw in text_lower)
        negative_count = sum(1 for kw in negative_keywords if kw in text_lower)

        total = positive_count + negative_count
        if total == 0:
            return SentimentResponse(
                sentiment="neutral",
                score=0.0,
                confidence=0.3,
                reasoning="No strong sentiment keywords found"
            )

        score = (positive_count - negative_count) / total

        if score > 0.2:
            sentiment = "positive"
        elif score < -0.2:
            sentiment = "negative"
        else:
            sentiment = "neutral"

        confidence = min(abs(score), 0.7)  # Keyword analysis is less confident

        reasoning = (
            f"Keyword analysis: {positive_count} positive, "
            f"{negative_count} negative keywords"
        )

        return SentimentResponse(
            sentiment=sentiment,
            score=score,
            confidence=confidence,
            reasoning=reasoning
        )

    def _preprocess_text(self, text: str) -> str:
        """Preprocess text for analysis"""

        # Remove URLs
        text = re.sub(r'http\S+', '', text)

        # Remove extra whitespace
        text = ' '.join(text.split())

        # Truncate if too long
        if len(text) > 512:
            text = text[:512]

        return text


# Global analyzer instance
analyzer = FinBERTAnalyzer()


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {
        "status": "healthy",
        "transformers_available": TRANSFORMERS_AVAILABLE,
        "model_loaded": analyzer.model is not None,
        "device": str(analyzer.device) if analyzer.device else "N/A"
    }


@app.post("/analyze_sentiment", response_model=SentimentResponse)
async def analyze_sentiment(request: SentimentRequest):
    """Analyze sentiment of text"""

    try:
        if not request.text or len(request.text.strip()) == 0:
            raise HTTPException(status_code=400, detail="Text cannot be empty")

        result = analyzer.analyze(request.text)
        logger.info(f"Analyzed sentiment: {result.sentiment} (score: {result.score:.2f})")
        return result

    except Exception as e:
        logger.error(f"Sentiment analysis failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/")
async def root():
    """Root endpoint"""
    return {
        "service": "FinBERT Sentiment Analysis",
        "version": "1.0.0",
        "model": "ProsusAI/finbert" if analyzer.model else "keyword-based",
        "endpoints": {
            "health": "/health",
            "analyze_sentiment": "/analyze_sentiment (POST)"
        }
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8002)
