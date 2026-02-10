"""Bayesian Adaptive Strategy Weights with Thompson Sampling."""
import numpy as np
from typing import Dict, List, Tuple
from dataclasses import dataclass
from datetime import datetime, timedelta
from loguru import logger
import json


@dataclass
class StrategyStats:
    """Statistics for a trading strategy."""
    name: str
    alpha: float  # Beta distribution parameter (wins)
    beta: float  # Beta distribution parameter (losses)
    total_samples: int
    win_rate: float
    weight: float
    last_updated: datetime


class BayesianStrategyWeights:
    """
    Bayesian adaptive strategy weighting using Beta-Bernoulli conjugate prior.

    This implements online learning of strategy performance with Thompson sampling
    for exploration-exploitation balance.
    """

    def __init__(
        self,
        prior_alpha: float = 1.0,
        prior_beta: float = 1.0,
        decay_factor: float = 0.95,
        min_samples: int = 10,
        exploration_rate: float = 0.1
    ):
        """
        Initialize Bayesian strategy weights.

        Args:
            prior_alpha: Prior wins (default 1.0 for uniform prior)
            prior_beta: Prior losses (default 1.0 for uniform prior)
            decay_factor: Exponential decay for older trades (0.95 = 5% decay)
            min_samples: Minimum samples before updating weights
            exploration_rate: Probability of exploration in Thompson sampling
        """
        self.prior_alpha = prior_alpha
        self.prior_beta = prior_beta
        self.decay_factor = decay_factor
        self.min_samples = min_samples
        self.exploration_rate = exploration_rate

        # Strategy statistics
        self.strategies: Dict[str, StrategyStats] = {}

        logger.info(f"Initialized Bayesian strategy weights (prior: α={prior_alpha}, β={prior_beta})")

    def initialize_strategy(self, strategy_name: str):
        """Initialize a new strategy with prior."""
        if strategy_name not in self.strategies:
            self.strategies[strategy_name] = StrategyStats(
                name=strategy_name,
                alpha=self.prior_alpha,
                beta=self.prior_beta,
                total_samples=0,
                win_rate=0.5,  # Neutral prior
                weight=1.0,
                last_updated=datetime.utcnow()
            )
            logger.info(f"Initialized strategy: {strategy_name}")

    def update_strategy(self, strategy_name: str, outcome: int, profit_loss: float = None):
        """
        Update strategy statistics with new outcome.

        Args:
            strategy_name: Name of the strategy
            outcome: 1 for win, 0 for loss
            profit_loss: Optional profit/loss amount for weighted updates
        """
        self.initialize_strategy(strategy_name)
        stats = self.strategies[strategy_name]

        # Apply decay to existing parameters
        time_since_update = (datetime.utcnow() - stats.last_updated).total_seconds() / 86400  # days
        decay = self.decay_factor ** time_since_update

        # Update with decay
        stats.alpha = stats.alpha * decay
        stats.beta = stats.beta * decay

        # Add new observation
        if outcome == 1:
            stats.alpha += 1.0
        else:
            stats.beta += 1.0

        # Update statistics
        stats.total_samples += 1
        stats.win_rate = stats.alpha / (stats.alpha + stats.beta)
        stats.last_updated = datetime.utcnow()

        logger.debug(f"Updated {strategy_name}: α={stats.alpha:.2f}, β={stats.beta:.2f}, "
                    f"win_rate={stats.win_rate:.3f}")

    def get_weights(self, normalize: bool = True) -> Dict[str, float]:
        """
        Get current strategy weights based on posterior mean.

        Args:
            normalize: Whether to normalize weights to sum to 1

        Returns:
            Dictionary of strategy names to weights
        """
        weights = {}

        for name, stats in self.strategies.items():
            # Use posterior mean (expected win rate) as weight
            if stats.total_samples >= self.min_samples:
                weights[name] = stats.win_rate
            else:
                # Use prior mean for strategies with insufficient data
                weights[name] = 0.5

        if normalize and weights:
            total = sum(weights.values())
            if total > 0:
                weights = {k: v / total for k, v in weights.items()}

        return weights

    def thompson_sampling(self, strategy_names: List[str], n_samples: int = 1) -> List[str]:
        """
        Select strategies using Thompson sampling.

        This balances exploration (trying uncertain strategies) with exploitation
        (using known good strategies).

        Args:
            strategy_names: List of available strategies
            n_samples: Number of strategies to sample

        Returns:
            List of selected strategy names
        """
        # Initialize strategies if needed
        for name in strategy_names:
            self.initialize_strategy(name)

        # Sample from posterior Beta distributions
        samples = {}
        for name in strategy_names:
            stats = self.strategies[name]

            # Sample from Beta(alpha, beta)
            sampled_win_rate = np.random.beta(stats.alpha, stats.beta)
            samples[name] = sampled_win_rate

        # With probability exploration_rate, select randomly
        if np.random.random() < self.exploration_rate:
            selected = np.random.choice(strategy_names, size=min(n_samples, len(strategy_names)), replace=False)
            return list(selected)

        # Otherwise, select top samples
        sorted_strategies = sorted(samples.items(), key=lambda x: x[1], reverse=True)
        selected = [name for name, _ in sorted_strategies[:n_samples]]

        return selected

    def get_credible_intervals(self, credibility: float = 0.95) -> Dict[str, Tuple[float, float]]:
        """
        Get credible intervals for strategy win rates.

        Args:
            credibility: Credibility level (e.g., 0.95 for 95% CI)

        Returns:
            Dictionary of strategy names to (lower, upper) credible intervals
        """
        intervals = {}
        alpha_level = (1 - credibility) / 2

        for name, stats in self.strategies.items():
            from scipy.stats import beta
            lower = beta.ppf(alpha_level, stats.alpha, stats.beta)
            upper = beta.ppf(1 - alpha_level, stats.alpha, stats.beta)
            intervals[name] = (lower, upper)

        return intervals

    def get_strategy_stats(self) -> Dict[str, Dict]:
        """Get detailed statistics for all strategies."""
        stats = {}
        for name, strategy in self.strategies.items():
            stats[name] = {
                "alpha": strategy.alpha,
                "beta": strategy.beta,
                "total_samples": strategy.total_samples,
                "win_rate": strategy.win_rate,
                "weight": strategy.weight,
                "last_updated": strategy.last_updated.isoformat()
            }
        return stats

    def load_from_database(self, db_data: Dict[str, Dict]):
        """
        Load strategy statistics from database.

        Args:
            db_data: Dictionary of strategy data from database
        """
        for name, data in db_data.items():
            self.strategies[name] = StrategyStats(
                name=name,
                alpha=data.get('alpha', self.prior_alpha),
                beta=data.get('beta', self.prior_beta),
                total_samples=data.get('total_samples', 0),
                win_rate=data.get('win_rate', 0.5),
                weight=data.get('weight', 1.0),
                last_updated=datetime.fromisoformat(data.get('last_updated', datetime.utcnow().isoformat()))
            )

        logger.info(f"Loaded {len(self.strategies)} strategies from database")

    def get_recommendation(self, strategy_name: str) -> Dict:
        """
        Get a recommendation on whether to use a strategy.

        Returns confidence, expected win rate, and credible interval.
        """
        if strategy_name not in self.strategies:
            return {
                "use_strategy": False,
                "reason": "Strategy not initialized",
                "confidence": 0.0
            }

        stats = self.strategies[strategy_name]

        # Don't recommend if insufficient samples
        if stats.total_samples < self.min_samples:
            return {
                "use_strategy": True,  # Allow during exploration phase
                "reason": f"Insufficient data ({stats.total_samples}/{self.min_samples} samples)",
                "confidence": 0.3,
                "expected_win_rate": stats.win_rate,
                "samples": stats.total_samples
            }

        # Compute credible interval
        from scipy.stats import beta
        lower = beta.ppf(0.025, stats.alpha, stats.beta)
        upper = beta.ppf(0.975, stats.alpha, stats.beta)

        # Recommend if lower bound of 95% CI is above 50%
        use_strategy = lower > 0.5

        # Confidence is width of credible interval (narrower = more confident)
        ci_width = upper - lower
        confidence = 1.0 - min(ci_width, 1.0)

        return {
            "use_strategy": use_strategy,
            "reason": f"Win rate: {stats.win_rate:.1%} (95% CI: {lower:.1%}-{upper:.1%})",
            "confidence": confidence,
            "expected_win_rate": stats.win_rate,
            "credible_interval": (lower, upper),
            "samples": stats.total_samples
        }

    def reset_strategy(self, strategy_name: str):
        """Reset a strategy to prior."""
        if strategy_name in self.strategies:
            self.strategies[strategy_name] = StrategyStats(
                name=strategy_name,
                alpha=self.prior_alpha,
                beta=self.prior_beta,
                total_samples=0,
                win_rate=0.5,
                weight=1.0,
                last_updated=datetime.utcnow()
            )
            logger.info(f"Reset strategy: {strategy_name}")

    def save_state(self) -> Dict:
        """Save current state to dictionary."""
        return {
            "config": {
                "prior_alpha": self.prior_alpha,
                "prior_beta": self.prior_beta,
                "decay_factor": self.decay_factor,
                "min_samples": self.min_samples,
                "exploration_rate": self.exploration_rate
            },
            "strategies": self.get_strategy_stats()
        }

    def load_state(self, state: Dict):
        """Load state from dictionary."""
        if "config" in state:
            cfg = state["config"]
            self.prior_alpha = cfg.get("prior_alpha", self.prior_alpha)
            self.prior_beta = cfg.get("prior_beta", self.prior_beta)
            self.decay_factor = cfg.get("decay_factor", self.decay_factor)
            self.min_samples = cfg.get("min_samples", self.min_samples)
            self.exploration_rate = cfg.get("exploration_rate", self.exploration_rate)

        if "strategies" in state:
            self.load_from_database(state["strategies"])

        logger.info("Loaded Bayesian state")
