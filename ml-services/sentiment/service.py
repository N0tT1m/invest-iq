"""FinBERT Sentiment Analysis FastAPI Service."""
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import List, Optional
import sys
from pathlib import Path
from loguru import logger

# Add parent directory to path
sys.path.append(str(Path(__file__).parent.parent))

from shared.config import config
from shared.database import MLDatabase
from sentiment.model import FinBERTSentiment, SentimentCache


# Request/Response Models
class SentimentRequest(BaseModel):
    texts: List[str] = Field(..., description="List of texts to analyze")
    symbol: Optional[str] = Field(None, description="Stock symbol for logging")
    use_cache: bool = Field(True, description="Use cached results if available")


class SentimentPrediction(BaseModel):
    label: str
    positive: float
    negative: float
    neutral: float
    confidence: float
    score: float


class SentimentResponse(BaseModel):
    predictions: List[SentimentPrediction]
    processing_time_ms: float


class NewsSentimentRequest(BaseModel):
    headlines: List[str]
    descriptions: Optional[List[str]] = None
    symbol: Optional[str] = None


class NewsSentimentResponse(BaseModel):
    overall_sentiment: str
    score: float
    confidence: float
    positive_ratio: float
    negative_ratio: float
    neutral_ratio: float
    article_count: int
    processing_time_ms: float


class HealthResponse(BaseModel):
    status: str
    model: str
    device: str
    cache_size: int


# Initialize FastAPI app
app = FastAPI(
    title="FinBERT Sentiment Analysis Service",
    description="Production-ready financial sentiment analysis using FinBERT",
    version="1.0.0"
)

# Global model and cache instances
model: Optional[FinBERTSentiment] = None
cache: Optional[SentimentCache] = None
db: Optional[MLDatabase] = None


@app.on_event("startup")
async def startup_event():
    """Initialize model on startup."""
    global model, cache, db

    logger.info("Initializing FinBERT sentiment service...")

    # Initialize model
    model = FinBERTSentiment(
        model_name=config.sentiment.model_name,
        device=config.gpu.device,
        quantize=config.sentiment.quantize,
        compile=config.gpu.compile,
        cache_dir=config.sentiment.cache_dir
    )

    # Initialize cache
    cache = SentimentCache(
        max_size=1000,
        ttl_seconds=config.inference.cache_ttl_seconds
    )

    # Initialize database
    db = MLDatabase(config.database_path)

    logger.info("FinBERT sentiment service ready!")


@app.on_event("shutdown")
async def shutdown_event():
    """Cleanup on shutdown."""
    logger.info("Shutting down FinBERT sentiment service...")
    if cache:
        cache.clear()


@app.get("/health", response_model=HealthResponse)
async def health():
    """Health check endpoint."""
    return HealthResponse(
        status="healthy",
        model=config.sentiment.model_name,
        device=str(model.device) if model else "unknown",
        cache_size=len(cache.cache) if cache else 0
    )


@app.post("/predict", response_model=SentimentResponse)
async def predict_sentiment(request: SentimentRequest):
    """
    Predict sentiment for one or more texts.

    This endpoint analyzes financial text and returns sentiment predictions
    with confidence scores. Results are cached for performance.
    """
    if not model:
        raise HTTPException(status_code=503, detail="Model not loaded")

    import time
    start_time = time.time()

    try:
        # Check cache if enabled
        cached_results = []
        uncached_texts = []
        text_indices = []

        if request.use_cache and cache:
            for i, text in enumerate(request.texts):
                cached = cache.get(text)
                if cached:
                    cached_results.append((i, cached))
                else:
                    uncached_texts.append(text)
                    text_indices.append(i)
        else:
            uncached_texts = request.texts
            text_indices = list(range(len(request.texts)))

        # Predict uncached texts
        predictions = []
        if uncached_texts:
            predictions = model.predict(uncached_texts, batch_size=config.sentiment.batch_size)

            # Cache new predictions
            if cache:
                for text, pred in zip(uncached_texts, predictions):
                    cache.set(text, pred)

        # Merge cached and new predictions
        all_predictions = [None] * len(request.texts)
        for i, pred in cached_results:
            all_predictions[i] = pred
        for i, pred in zip(text_indices, predictions):
            all_predictions[i] = pred

        # Log to database if symbol provided
        if request.symbol and db:
            for text, pred in zip(request.texts, all_predictions):
                db.log_sentiment_prediction(
                    symbol=request.symbol,
                    text=text[:500],  # Truncate long texts
                    label=pred["label"],
                    score=pred["score"],
                    confidence=pred["confidence"],
                    model_version="finbert-v1"
                )

        processing_time = (time.time() - start_time) * 1000

        return SentimentResponse(
            predictions=[SentimentPrediction(**p) for p in all_predictions],
            processing_time_ms=processing_time
        )

    except Exception as e:
        logger.error(f"Prediction error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/analyze-news", response_model=NewsSentimentResponse)
async def analyze_news(request: NewsSentimentRequest):
    """
    Analyze sentiment from multiple news articles and compute aggregate metrics.

    This endpoint is optimized for analyzing news headlines and descriptions
    to provide an overall market sentiment score for a symbol.
    """
    if not model:
        raise HTTPException(status_code=503, detail="Model not loaded")

    import time
    start_time = time.time()

    try:
        # Analyze news
        result = model.analyze_news(
            headlines=request.headlines,
            descriptions=request.descriptions
        )

        # Log aggregate sentiment
        if request.symbol and db:
            # Create summary text
            summary = f"{len(request.headlines)} news articles"
            db.log_sentiment_prediction(
                symbol=request.symbol,
                text=summary,
                label=result["overall_sentiment"],
                score=result["score"],
                confidence=result["confidence"],
                model_version="finbert-v1"
            )

        processing_time = (time.time() - start_time) * 1000

        return NewsSentimentResponse(
            overall_sentiment=result["overall_sentiment"],
            score=result["score"],
            confidence=result["confidence"],
            positive_ratio=result["positive_ratio"],
            negative_ratio=result["negative_ratio"],
            neutral_ratio=result["neutral_ratio"],
            article_count=result["article_count"],
            processing_time_ms=processing_time
        )

    except Exception as e:
        logger.error(f"News analysis error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/clear-cache")
async def clear_cache():
    """Clear the prediction cache."""
    if cache:
        cache.clear()
        return {"status": "success", "message": "Cache cleared"}
    return {"status": "error", "message": "Cache not initialized"}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        app,
        host=config.service.host,
        port=config.service.port_sentiment,
        log_level=config.service.log_level
    )
