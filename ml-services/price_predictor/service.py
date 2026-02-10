"""Price Direction Predictor FastAPI Service."""
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import List, Dict, Optional
import sys
from pathlib import Path
from loguru import logger
import numpy as np

sys.path.append(str(Path(__file__).parent.parent))

from shared.config import config
from shared.database import MLDatabase
from price_predictor.model import PricePredictorInference


# Request/Response Models
class PriceData(BaseModel):
    """Single timestep of price data."""
    open: float
    high: float
    low: float
    close: float
    volume: float
    vwap: Optional[float] = None


class PredictionRequest(BaseModel):
    symbol: str = Field(..., description="Stock symbol")
    history: List[PriceData] = Field(..., description="Historical price data")
    horizon_steps: int = Field(1, description="Number of future steps to predict", ge=1, le=12)


class DirectionPrediction(BaseModel):
    direction: str  # up, down, neutral
    confidence: float
    probabilities: Dict[str, float]
    horizon_steps: int
    predicted_prices: List[float]


class BatchPredictionRequest(BaseModel):
    predictions: List[PredictionRequest]


class HealthResponse(BaseModel):
    status: str
    model_path: str
    device: str
    context_length: int
    prediction_length: int


# Initialize FastAPI app
app = FastAPI(
    title="PatchTST Price Direction Predictor",
    description="Deep learning price direction prediction using PatchTST",
    version="1.0.0"
)

# Global model instance
predictor: Optional[PricePredictorInference] = None
db: Optional[MLDatabase] = None


@app.on_event("startup")
async def startup_event():
    """Initialize model on startup."""
    global predictor, db

    logger.info("Initializing price predictor service...")

    # Check if trained model exists
    model_path = Path(config.price_predictor.cache_dir) / "trained"
    if not model_path.exists() or not (model_path / "model.pt").exists():
        logger.warning(f"Trained model not found at {model_path}")
        logger.warning("Please train the model first using: python price_predictor/train.py")
        # Initialize with None - service will return appropriate errors
        predictor = None
    else:
        # Initialize predictor
        predictor = PricePredictorInference(
            model_path=str(model_path),
            device=config.gpu.device,
            compile=config.gpu.compile
        )
        logger.info("Price predictor loaded successfully")

    # Initialize database
    db = MLDatabase(config.database_path)

    logger.info("Price predictor service ready!")


@app.get("/health", response_model=HealthResponse)
async def health():
    """Health check endpoint."""
    if not predictor:
        return HealthResponse(
            status="model_not_loaded",
            model_path="N/A",
            device="N/A",
            context_length=0,
            prediction_length=0
        )

    return HealthResponse(
        status="healthy",
        model_path=str(predictor.model_path),
        device=str(predictor.device),
        context_length=predictor.config['context_length'],
        prediction_length=predictor.config['prediction_length']
    )


@app.post("/predict", response_model=DirectionPrediction)
async def predict_direction(request: PredictionRequest):
    """
    Predict price direction for the next N steps.

    This endpoint uses PatchTST to predict future price movements with confidence scores.
    Useful for confirming trading signals.
    """
    if not predictor:
        raise HTTPException(
            status_code=503,
            detail="Model not loaded. Please train the model first."
        )

    try:
        # Validate history length
        required_length = predictor.config['context_length']
        if len(request.history) < required_length:
            raise HTTPException(
                status_code=400,
                detail=f"Insufficient history. Need {required_length} data points, got {len(request.history)}"
            )

        # Convert to numpy array
        features = predictor.config.get('features', ['open', 'high', 'low', 'close', 'volume', 'vwap'])
        history_array = np.zeros((len(request.history), len(features)))

        for i, data_point in enumerate(request.history):
            history_array[i, 0] = data_point.open
            history_array[i, 1] = data_point.high
            history_array[i, 2] = data_point.low
            history_array[i, 3] = data_point.close
            history_array[i, 4] = data_point.volume
            history_array[i, 5] = data_point.vwap or data_point.close

        # Take last context_length points
        history_array = history_array[-required_length:]

        # Predict
        result = predictor.predict_next_direction(
            history=history_array,
            horizon_steps=request.horizon_steps
        )

        # Log to database
        if db:
            db.log_price_prediction(
                symbol=request.symbol,
                timeframe="15m",  # Configurable
                horizon=request.horizon_steps,
                direction=result['direction'],
                predicted_price=result['predicted_prices'][0] if result['predicted_prices'] else 0.0,
                confidence=result['confidence'],
                current_price=request.history[-1].close,
                features={'horizon_steps': request.horizon_steps},
                model_version="patchtst-v1"
            )

        return DirectionPrediction(**result)

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Prediction error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/batch-predict")
async def batch_predict(request: BatchPredictionRequest):
    """Predict for multiple symbols in batch."""
    if not predictor:
        raise HTTPException(
            status_code=503,
            detail="Model not loaded. Please train the model first."
        )

    try:
        results = []
        for pred_request in request.predictions:
            # Process each prediction
            result = await predict_direction(pred_request)
            results.append({
                "symbol": pred_request.symbol,
                "prediction": result
            })

        return {"predictions": results}

    except Exception as e:
        logger.error(f"Batch prediction error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/evaluate/{symbol}")
async def evaluate_predictions(symbol: str, days: int = 7):
    """
    Evaluate prediction accuracy for a symbol.

    Returns metrics on how well the model has been predicting.
    """
    if not db:
        raise HTTPException(status_code=503, detail="Database not initialized")

    try:
        metrics = db.evaluate_predictions(
            model_name="price_predictor",
            symbol=symbol,
            days=days
        )

        return {
            "symbol": symbol,
            "days": days,
            "metrics": metrics
        }

    except Exception as e:
        logger.error(f"Evaluation error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/model-info")
async def get_model_info():
    """Get detailed model information."""
    if not predictor:
        raise HTTPException(
            status_code=503,
            detail="Model not loaded. Please train the model first."
        )

    return {
        "model_path": str(predictor.model_path),
        "config": predictor.config,
        "normalization_stats": predictor.norm_stats,
        "device": str(predictor.device)
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        app,
        host=config.service.host,
        port=config.service.port_price_predictor,
        log_level=config.service.log_level
    )
