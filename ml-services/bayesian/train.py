"""Bootstrap Bayesian strategy weights from backtest trade history.

Reads backtest_trades from portfolio.db, groups trades by signal type,
feeds win/loss outcomes into BayesianStrategyWeights, and saves final
state to the strategy_weights table.
"""
from dotenv import load_dotenv
load_dotenv()
load_dotenv(dotenv_path="../.env")
import argparse
import sqlite3
import sys
from pathlib import Path
from loguru import logger

sys.path.append(str(Path(__file__).parent.parent))
from shared.config import config
from shared.database import MLDatabase
from bayesian.model import BayesianStrategyWeights


def load_backtest_trades(db_path: str) -> list[dict]:
    """Load completed backtest trades from the DB."""
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    cursor.execute("""
        SELECT symbol, signal, entry_price, exit_price,
               profit_loss, profit_loss_percent, holding_period_days
        FROM backtest_trades
        ORDER BY entry_date ASC
    """)
    rows = [dict(r) for r in cursor.fetchall()]
    conn.close()
    return rows


def load_real_trades(db_path: str) -> list[dict]:
    """Load real (paper/live) trades from the DB."""
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    cursor.execute("""
        SELECT symbol, side, profit_loss, trade_date
        FROM trades
        WHERE profit_loss IS NOT NULL
        ORDER BY trade_date ASC
    """)
    rows = [dict(r) for r in cursor.fetchall()]
    conn.close()
    return rows


def bootstrap(db_path: str):
    """Bootstrap Bayesian weights from historical trades."""
    logger.info(f"Loading trades from {db_path}")

    backtest_trades = load_backtest_trades(db_path)
    real_trades = load_real_trades(db_path)

    logger.info(f"Backtest trades: {len(backtest_trades)}")
    logger.info(f"Real trades: {len(real_trades)}")

    if not backtest_trades and not real_trades:
        logger.warning("No trades found. Run some backtests first.")
        return

    model = BayesianStrategyWeights(
        prior_alpha=config.bayesian.prior_alpha,
        prior_beta=config.bayesian.prior_beta,
        decay_factor=config.bayesian.decay_factor,
        min_samples=config.bayesian.min_samples,
        exploration_rate=config.bayesian.exploration_rate,
    )

    # Feed backtest trades grouped by signal type as strategy names
    for trade in backtest_trades:
        signal = trade.get("signal", "Neutral")
        strategy_name = f"signal_{signal}"
        pnl = trade.get("profit_loss_percent", 0) or 0
        outcome = 1 if pnl > 0 else 0
        model.update_strategy(strategy_name, outcome, profit_loss=pnl)

    # Also create per-symbol strategies from backtests
    for trade in backtest_trades:
        symbol = trade.get("symbol", "UNKNOWN")
        strategy_name = f"symbol_{symbol}"
        pnl = trade.get("profit_loss_percent", 0) or 0
        outcome = 1 if pnl > 0 else 0
        model.update_strategy(strategy_name, outcome, profit_loss=pnl)

    # Feed real trades as "live" strategy
    for trade in real_trades:
        pnl = trade.get("profit_loss", 0) or 0
        outcome = 1 if pnl > 0 else 0
        model.update_strategy("live_trading", outcome, profit_loss=pnl)

    # Save to database
    db = MLDatabase(db_path)
    stats = model.get_strategy_stats()
    for name, strategy_stats in stats.items():
        db.update_strategy_weight(
            strategy_name=name,
            weight=strategy_stats["weight"],
            alpha=strategy_stats["alpha"],
            beta=strategy_stats["beta"],
            win_rate=strategy_stats["win_rate"],
            total_samples=strategy_stats["total_samples"],
        )

    # Report
    logger.info(f"\nBootstrapped {len(stats)} strategies:")
    weights = model.get_weights(normalize=True)
    for name in sorted(weights, key=lambda k: weights[k], reverse=True):
        s = model.strategies[name]
        logger.info(
            f"  {name:30s}  win_rate={s.win_rate:.1%}  "
            f"samples={s.total_samples:4d}  weight={weights[name]:.3f}"
        )


def main():
    parser = argparse.ArgumentParser(description="Bootstrap Bayesian strategy weights from trade history")
    parser.add_argument("--db-path", default="../portfolio.db", help="Path to portfolio.db")
    args = parser.parse_args()
    bootstrap(args.db_path)


if __name__ == "__main__":
    main()
