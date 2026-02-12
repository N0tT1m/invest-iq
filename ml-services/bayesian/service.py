"""Bayesian Strategy Weights FastAPI Service."""
from fastapi import FastAPI, HTTPException, BackgroundTasks
from pydantic import BaseModel, Field
from typing import List, Dict, Optional, Tuple
import sys
from pathlib import Path
from loguru import logger
import asyncio

sys.path.append(str(Path(__file__).parent.parent))

from shared.config import config
from shared.database import MLDatabase
from bayesian.model import BayesianStrategyWeights


# Request/Response Models
class UpdateRequest(BaseModel):
    strategy_name: str = Field(..., description="Name of the strategy")
    outcome: int = Field(..., description="1 for win, 0 for loss")
    profit_loss: Optional[float] = Field(None, description="Profit/loss amount")
    trade_id: Optional[int] = Field(None, description="Trade ID for tracking")


class WeightsResponse(BaseModel):
    weights: Dict[str, float]
    normalized: bool


class ThompsonSamplingRequest(BaseModel):
    strategies: List[str] = Field(..., description="List of available strategies")
    n_samples: int = Field(1, description="Number of strategies to select", ge=1)


class ThompsonSamplingResponse(BaseModel):
    selected_strategies: List[str]
    all_weights: Dict[str, float]


class StrategyStatsResponse(BaseModel):
    strategy_name: str
    alpha: float
    beta: float
    total_samples: int
    win_rate: float
    weight: float
    credible_interval: Optional[Tuple[float, float]] = None


class RecommendationResponse(BaseModel):
    use_strategy: bool
    reason: str
    confidence: float
    expected_win_rate: Optional[float] = None
    credible_interval: Optional[Tuple[float, float]] = None
    samples: Optional[int] = None


class BatchUpdateRequest(BaseModel):
    updates: List[UpdateRequest]


class HealthResponse(BaseModel):
    status: str
    num_strategies: int
    total_samples: int


# Initialize FastAPI app
app = FastAPI(
    title="Bayesian Strategy Weights Service",
    description="Online learning of strategy performance using Bayesian methods",
    version="1.0.0"
)

# Production hardening middleware
from shared.middleware import setup_hardening
setup_hardening(app, "bayesian")

# Global model instance
bayesian_model: Optional[BayesianStrategyWeights] = None
db: Optional[MLDatabase] = None


@app.on_event("startup")
async def startup_event():
    """Initialize model on startup."""
    global bayesian_model, db

    logger.info("Initializing Bayesian strategy weights service...")

    # Initialize database
    db = MLDatabase(config.database_path)

    # Initialize Bayesian model
    bayesian_model = BayesianStrategyWeights(
        prior_alpha=config.bayesian.prior_alpha,
        prior_beta=config.bayesian.prior_beta,
        decay_factor=config.bayesian.decay_factor,
        min_samples=config.bayesian.min_samples,
        exploration_rate=config.bayesian.exploration_rate
    )

    # Load existing weights from database
    existing_weights = db.get_strategy_weights()
    if existing_weights:
        bayesian_model.load_from_database(existing_weights)
        logger.info(f"Loaded {len(existing_weights)} strategies from database")

    logger.info("Bayesian strategy weights service ready!")


@app.on_event("shutdown")
async def shutdown_event():
    """Save state on shutdown."""
    logger.info("Shutting down Bayesian strategy weights service...")
    if bayesian_model and db:
        # Save final state to database
        stats = bayesian_model.get_strategy_stats()
        for name, strategy_stats in stats.items():
            db.update_strategy_weight(
                strategy_name=name,
                weight=strategy_stats['weight'],
                alpha=strategy_stats['alpha'],
                beta=strategy_stats['beta'],
                win_rate=strategy_stats['win_rate'],
                total_samples=strategy_stats['total_samples']
            )
        logger.info("Saved final state to database")


def save_to_database_sync():
    """Synchronously save state to database."""
    if bayesian_model and db:
        stats = bayesian_model.get_strategy_stats()
        for name, strategy_stats in stats.items():
            db.update_strategy_weight(
                strategy_name=name,
                weight=strategy_stats['weight'],
                alpha=strategy_stats['alpha'],
                beta=strategy_stats['beta'],
                win_rate=strategy_stats['win_rate'],
                total_samples=strategy_stats['total_samples']
            )


@app.get("/health", response_model=HealthResponse)
async def health():
    """Health check endpoint."""
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    total_samples = sum(
        s.total_samples for s in bayesian_model.strategies.values()
    )

    return HealthResponse(
        status="healthy",
        num_strategies=len(bayesian_model.strategies),
        total_samples=total_samples
    )


@app.post("/update")
async def update_strategy(request: UpdateRequest, background_tasks: BackgroundTasks):
    """
    Update a strategy with a new trade outcome.

    This endpoint updates the Bayesian posterior for a strategy based on
    whether the trade was a win or loss.
    """
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        # Update Bayesian model
        bayesian_model.update_strategy(
            strategy_name=request.strategy_name,
            outcome=request.outcome,
            profit_loss=request.profit_loss
        )

        # Log to database
        if db:
            db.log_strategy_outcome(
                strategy_name=request.strategy_name,
                outcome=request.outcome,
                profit_loss=request.profit_loss,
                trade_id=request.trade_id
            )

            # Save weights to database in background
            background_tasks.add_task(save_to_database_sync)

        # Get updated stats
        stats = bayesian_model.strategies[request.strategy_name]

        return {
            "status": "success",
            "strategy_name": request.strategy_name,
            "updated_stats": {
                "alpha": stats.alpha,
                "beta": stats.beta,
                "win_rate": stats.win_rate,
                "total_samples": stats.total_samples
            }
        }

    except Exception as e:
        logger.error(f"Update error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/batch-update")
async def batch_update(request: BatchUpdateRequest, background_tasks: BackgroundTasks):
    """Update multiple strategies at once."""
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        results = []
        for update in request.updates:
            bayesian_model.update_strategy(
                strategy_name=update.strategy_name,
                outcome=update.outcome,
                profit_loss=update.profit_loss
            )

            if db:
                db.log_strategy_outcome(
                    strategy_name=update.strategy_name,
                    outcome=update.outcome,
                    profit_loss=update.profit_loss,
                    trade_id=update.trade_id
                )

            stats = bayesian_model.strategies[update.strategy_name]
            results.append({
                "strategy_name": update.strategy_name,
                "win_rate": stats.win_rate,
                "total_samples": stats.total_samples
            })

        # Save to database in background
        background_tasks.add_task(save_to_database_sync)

        return {
            "status": "success",
            "updated_count": len(results),
            "results": results
        }

    except Exception as e:
        logger.error(f"Batch update error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/weights", response_model=WeightsResponse)
async def get_weights(normalize: bool = True):
    """
    Get current strategy weights.

    Weights are based on the posterior mean (expected win rate) of each strategy.
    """
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        weights = bayesian_model.get_weights(normalize=normalize)
        return WeightsResponse(weights=weights, normalized=normalize)

    except Exception as e:
        logger.error(f"Get weights error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/thompson-sampling", response_model=ThompsonSamplingResponse)
async def thompson_sampling(request: ThompsonSamplingRequest):
    """
    Select strategies using Thompson sampling.

    This balances exploration (trying uncertain strategies) with exploitation
    (using strategies with proven performance).
    """
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        selected = bayesian_model.thompson_sampling(
            strategy_names=request.strategies,
            n_samples=request.n_samples
        )

        weights = bayesian_model.get_weights(normalize=True)

        return ThompsonSamplingResponse(
            selected_strategies=selected,
            all_weights=weights
        )

    except Exception as e:
        logger.error(f"Thompson sampling error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/strategy/{strategy_name}", response_model=StrategyStatsResponse)
async def get_strategy_stats(strategy_name: str, include_ci: bool = True):
    """Get detailed statistics for a specific strategy."""
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    if strategy_name not in bayesian_model.strategies:
        raise HTTPException(status_code=404, detail=f"Strategy '{strategy_name}' not found")

    try:
        stats = bayesian_model.strategies[strategy_name]

        ci = None
        if include_ci:
            intervals = bayesian_model.get_credible_intervals(credibility=0.95)
            ci = intervals.get(strategy_name)

        return StrategyStatsResponse(
            strategy_name=strategy_name,
            alpha=stats.alpha,
            beta=stats.beta,
            total_samples=stats.total_samples,
            win_rate=stats.win_rate,
            weight=stats.weight,
            credible_interval=ci
        )

    except Exception as e:
        logger.error(f"Get strategy stats error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/recommendation/{strategy_name}", response_model=RecommendationResponse)
async def get_recommendation(strategy_name: str):
    """
    Get a recommendation on whether to use a strategy.

    Returns whether to use the strategy, confidence level, and reasoning.
    """
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        recommendation = bayesian_model.get_recommendation(strategy_name)
        return RecommendationResponse(**recommendation)

    except Exception as e:
        logger.error(f"Get recommendation error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/all-stats")
async def get_all_stats():
    """Get statistics for all strategies."""
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        stats = bayesian_model.get_strategy_stats()
        intervals = bayesian_model.get_credible_intervals(credibility=0.95)

        result = []
        for name, strategy_stats in stats.items():
            result.append({
                "strategy_name": name,
                "win_rate": strategy_stats['win_rate'],
                "total_samples": strategy_stats['total_samples'],
                "weight": strategy_stats['weight'],
                "credible_interval": intervals.get(name, (0, 1))
            })

        # Sort by win rate
        result.sort(key=lambda x: x['win_rate'], reverse=True)

        return {"strategies": result}

    except Exception as e:
        logger.error(f"Get all stats error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/sync-from-database")
async def sync_from_database(days: int = 7):
    """
    Sync strategy weights from recent trades in database.

    This is useful for initializing or updating weights from historical data.
    """
    if not bayesian_model or not db:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        from datetime import datetime, timedelta

        # Get recent trades
        since = (datetime.utcnow() - timedelta(days=days)).isoformat()
        trades = db.get_trades_for_strategy_update(since_timestamp=since)

        updated_strategies = set()
        for trade in trades:
            if trade.get('strategy_name') and trade.get('profit_loss') is not None:
                outcome = 1 if trade['profit_loss'] > 0 else 0
                bayesian_model.update_strategy(
                    strategy_name=trade['strategy_name'],
                    outcome=outcome,
                    profit_loss=trade['profit_loss']
                )
                updated_strategies.add(trade['strategy_name'])

        # Save to database
        save_to_database_sync()

        return {
            "status": "success",
            "trades_processed": len(trades),
            "strategies_updated": list(updated_strategies),
            "days": days
        }

    except Exception as e:
        logger.error(f"Sync from database error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/reset/{strategy_name}")
async def reset_strategy(strategy_name: str):
    """Reset a strategy to prior (useful for retraining)."""
    if not bayesian_model:
        raise HTTPException(status_code=503, detail="Model not initialized")

    try:
        bayesian_model.reset_strategy(strategy_name)
        return {
            "status": "success",
            "strategy_name": strategy_name,
            "message": "Strategy reset to prior"
        }

    except Exception as e:
        logger.error(f"Reset strategy error: {e}")
        raise HTTPException(status_code=500, detail=str(e))


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        app,
        host=config.service.host,
        port=config.service.port_bayesian,
        log_level=config.service.log_level
    )
