"""
InvestIQ Dashboard Components

New feature visualization components for:
- Sentiment Velocity
- Risk Radar
- Confidence Compass (Calibration)
- Flow Map
- And more...
"""

from .sentiment_velocity import SentimentVelocityComponent, create_velocity_gauge
from .risk_radar import RiskRadarComponent, create_risk_radar_chart
from .confidence_gauge import (
    ConfidenceGaugeComponent,
    create_confidence_gauge,
    create_uncertainty_breakdown,
    create_reliability_diagram,
)
from .alpha_decay import (
    AlphaDecayComponent,
    create_sharpe_trend_chart,
    create_cusum_chart,
    create_health_score_gauge,
)
from .smart_watchlist import (
    SmartWatchlistComponent,
    create_relevance_distribution,
    create_signal_breakdown,
)
from .flow_map import (
    FlowMapComponent,
    create_sector_heatmap,
    create_flow_sankey,
    create_rotation_chart,
    create_relative_strength_radar,
)
from .tax_dashboard import (
    TaxDashboardComponent,
    create_savings_gauge,
    create_year_end_chart,
    create_tax_summary_card,
    create_wash_sale_calendar,
)
from .earnings_panel import EarningsPanelComponent
from .dividend_panel import DividendPanelComponent
from .options_flow import OptionsFlowComponent
from .short_interest import ShortInterestComponent
from .insider_activity import InsiderActivityComponent
from .correlation_matrix import CorrelationMatrixComponent
from .social_sentiment import SocialSentimentComponent
from .macro_overlay import MacroOverlayComponent
from .portfolio_dashboard import PortfolioDashboardComponent
from .backtest_panel import BacktestPanelComponent

__all__ = [
    'SentimentVelocityComponent',
    'create_velocity_gauge',
    'RiskRadarComponent',
    'create_risk_radar_chart',
    'ConfidenceGaugeComponent',
    'create_confidence_gauge',
    'create_uncertainty_breakdown',
    'create_reliability_diagram',
    'AlphaDecayComponent',
    'create_sharpe_trend_chart',
    'create_cusum_chart',
    'create_health_score_gauge',
    'SmartWatchlistComponent',
    'create_relevance_distribution',
    'create_signal_breakdown',
    'FlowMapComponent',
    'create_sector_heatmap',
    'create_flow_sankey',
    'create_rotation_chart',
    'create_relative_strength_radar',
    'TaxDashboardComponent',
    'create_savings_gauge',
    'create_year_end_chart',
    'create_tax_summary_card',
    'create_wash_sale_calendar',
    'EarningsPanelComponent',
    'DividendPanelComponent',
    'OptionsFlowComponent',
    'ShortInterestComponent',
    'InsiderActivityComponent',
    'CorrelationMatrixComponent',
    'SocialSentimentComponent',
    'MacroOverlayComponent',
    'PortfolioDashboardComponent',
    'BacktestPanelComponent',
]
