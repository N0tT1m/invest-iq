"""Database utilities for ML services."""
import sqlite3
from datetime import datetime
from typing import List, Dict, Any, Optional
from contextlib import contextmanager
from pathlib import Path
import json


class MLDatabase:
    """Database interface for ML predictions and model tracking."""

    def __init__(self, db_path: str = "../portfolio.db"):
        self.db_path = Path(db_path).resolve()
        self._init_schema()

    @contextmanager
    def get_connection(self):
        """Context manager for database connections."""
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        try:
            yield conn
            conn.commit()
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    def _init_schema(self):
        """Initialize ML-related database tables."""
        with self.get_connection() as conn:
            cursor = conn.cursor()

            # ML Predictions table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS ml_predictions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    model_name TEXT NOT NULL,
                    symbol TEXT NOT NULL,
                    prediction_type TEXT NOT NULL,
                    prediction_value REAL,
                    prediction_json TEXT,
                    confidence REAL,
                    features_json TEXT,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    actual_value REAL,
                    error REAL,
                    evaluated_at TEXT
                )
            """)

            # Sentiment predictions
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS sentiment_predictions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    symbol TEXT NOT NULL,
                    text TEXT NOT NULL,
                    sentiment_label TEXT NOT NULL,
                    sentiment_score REAL NOT NULL,
                    confidence REAL NOT NULL,
                    model_version TEXT,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP
                )
            """)

            # Strategy weights (Bayesian)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS strategy_weights (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    strategy_name TEXT NOT NULL,
                    weight REAL NOT NULL,
                    alpha REAL NOT NULL,
                    beta REAL NOT NULL,
                    win_rate REAL NOT NULL,
                    total_samples INTEGER NOT NULL,
                    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(strategy_name)
                )
            """)

            # Strategy performance history
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS strategy_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    strategy_name TEXT NOT NULL,
                    trade_id INTEGER,
                    outcome INTEGER NOT NULL,
                    profit_loss REAL,
                    confidence REAL,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP
                )
            """)

            # Price predictions
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS price_predictions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    symbol TEXT NOT NULL,
                    timeframe TEXT NOT NULL,
                    prediction_horizon INTEGER NOT NULL,
                    predicted_direction TEXT NOT NULL,
                    predicted_price REAL,
                    confidence REAL NOT NULL,
                    current_price REAL NOT NULL,
                    features_json TEXT,
                    model_version TEXT,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    actual_price REAL,
                    correct INTEGER,
                    evaluated_at TEXT
                )
            """)

            # Model metadata
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS model_metadata (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    model_name TEXT NOT NULL,
                    model_type TEXT NOT NULL,
                    version TEXT NOT NULL,
                    path TEXT NOT NULL,
                    metrics_json TEXT,
                    config_json TEXT,
                    trained_at TEXT DEFAULT CURRENT_TIMESTAMP,
                    is_active INTEGER DEFAULT 1,
                    UNIQUE(model_name, version)
                )
            """)

            # Analysis features (for signal model training)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS analysis_features (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    symbol TEXT NOT NULL,
                    analysis_date TEXT NOT NULL,
                    features_json TEXT NOT NULL,
                    overall_signal TEXT NOT NULL,
                    overall_confidence REAL NOT NULL,
                    actual_return_5d REAL,
                    actual_return_20d REAL,
                    evaluated INTEGER DEFAULT 0,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP
                )
            """)

            # Create indexes
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_ml_pred_symbol ON ml_predictions(symbol, created_at)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_sentiment_symbol ON sentiment_predictions(symbol, created_at)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_price_pred_symbol ON price_predictions(symbol, created_at)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_strategy_hist_name ON strategy_history(strategy_name, created_at)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_analysis_feat_symbol ON analysis_features(symbol, analysis_date)")

    def log_sentiment_prediction(self, symbol: str, text: str, label: str, score: float, confidence: float, model_version: str = "v1"):
        """Log a sentiment prediction."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO sentiment_predictions (symbol, text, sentiment_label, sentiment_score, confidence, model_version)
                VALUES (?, ?, ?, ?, ?, ?)
            """, (symbol, text, label, score, confidence, model_version))
            return cursor.lastrowid

    def log_price_prediction(self, symbol: str, timeframe: str, horizon: int, direction: str,
                            predicted_price: float, confidence: float, current_price: float,
                            features: Dict[str, Any], model_version: str = "v1"):
        """Log a price prediction."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO price_predictions (symbol, timeframe, prediction_horizon, predicted_direction,
                                              predicted_price, confidence, current_price, features_json, model_version)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (symbol, timeframe, horizon, direction, predicted_price, confidence, current_price,
                  json.dumps(features), model_version))
            return cursor.lastrowid

    def update_strategy_weight(self, strategy_name: str, weight: float, alpha: float, beta: float, win_rate: float, total_samples: int):
        """Update strategy weight (Bayesian)."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO strategy_weights (strategy_name, weight, alpha, beta, win_rate, total_samples, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(strategy_name) DO UPDATE SET
                    weight = ?,
                    alpha = ?,
                    beta = ?,
                    win_rate = ?,
                    total_samples = ?,
                    updated_at = ?
            """, (strategy_name, weight, alpha, beta, win_rate, total_samples, datetime.utcnow().isoformat(),
                  weight, alpha, beta, win_rate, total_samples, datetime.utcnow().isoformat()))

    def log_strategy_outcome(self, strategy_name: str, outcome: int, profit_loss: float = None,
                            confidence: float = None, trade_id: int = None):
        """Log a strategy outcome (1 for win, 0 for loss)."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO strategy_history (strategy_name, trade_id, outcome, profit_loss, confidence)
                VALUES (?, ?, ?, ?, ?)
            """, (strategy_name, trade_id, outcome, profit_loss, confidence))
            return cursor.lastrowid

    def get_strategy_history(self, strategy_name: str, limit: int = 100) -> List[Dict[str, Any]]:
        """Get recent strategy outcomes."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                SELECT * FROM strategy_history
                WHERE strategy_name = ?
                ORDER BY created_at DESC
                LIMIT ?
            """, (strategy_name, limit))
            return [dict(row) for row in cursor.fetchall()]

    def get_strategy_weights(self) -> Dict[str, Dict[str, float]]:
        """Get all strategy weights."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM strategy_weights")
            weights = {}
            for row in cursor.fetchall():
                weights[row['strategy_name']] = {
                    'weight': row['weight'],
                    'alpha': row['alpha'],
                    'beta': row['beta'],
                    'win_rate': row['win_rate'],
                    'total_samples': row['total_samples']
                }
            return weights

    def get_trades_for_strategy_update(self, since_timestamp: Optional[str] = None) -> List[Dict[str, Any]]:
        """Get trades for updating strategy weights."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            query = """
                SELECT t.id, t.symbol, t.profit_loss, t.trade_date,
                       bt.strategy_name, bt.confidence, bt.exit_reason
                FROM trades t
                LEFT JOIN backtest_trades bt ON t.symbol = bt.symbol
                WHERE t.profit_loss IS NOT NULL
            """
            params = []
            if since_timestamp:
                query += " AND t.trade_date > ?"
                params.append(since_timestamp)
            query += " ORDER BY t.trade_date DESC LIMIT 1000"

            cursor.execute(query, params)
            return [dict(row) for row in cursor.fetchall()]

    def save_model_metadata(self, model_name: str, model_type: str, version: str,
                           path: str, metrics: Dict[str, Any], config: Dict[str, Any]):
        """Save model metadata."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                INSERT INTO model_metadata (model_name, model_type, version, path, metrics_json, config_json)
                VALUES (?, ?, ?, ?, ?, ?)
            """, (model_name, model_type, version, path, json.dumps(metrics), json.dumps(config)))
            return cursor.lastrowid

    def get_active_model(self, model_name: str) -> Optional[Dict[str, Any]]:
        """Get active model metadata."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
                SELECT * FROM model_metadata
                WHERE model_name = ? AND is_active = 1
                ORDER BY trained_at DESC
                LIMIT 1
            """, (model_name,))
            row = cursor.fetchone()
            return dict(row) if row else None

    def evaluate_predictions(self, model_name: str, symbol: str = None, days: int = 7) -> Dict[str, float]:
        """Evaluate prediction accuracy."""
        with self.get_connection() as conn:
            cursor = conn.cursor()

            if model_name == "price_predictor":
                query = """
                    SELECT AVG(CASE WHEN correct = 1 THEN 1.0 ELSE 0.0 END) as accuracy,
                           AVG(ABS(predicted_price - actual_price)) as mae,
                           COUNT(*) as total
                    FROM price_predictions
                    WHERE actual_price IS NOT NULL
                    AND created_at > datetime('now', ? || ' days')
                """
                params = [-days]
                if symbol:
                    query += " AND symbol = ?"
                    params.append(symbol)

                cursor.execute(query, params)
                row = cursor.fetchone()
                return {
                    'accuracy': row['accuracy'] or 0.0,
                    'mae': row['mae'] or 0.0,
                    'total': row['total'] or 0
                }

        return {'accuracy': 0.0, 'mae': 0.0, 'total': 0}
