-- Fix max_portfolio_risk_percent default from 10% to 80%.
-- The old 10% value combined with the old buggy formula (count * risk_per_trade%)
-- was meant to limit theoretical risk, but it effectively capped the agent at 5 positions.
-- Now that check_trade_risk uses actual exposure (positions_value / total_value),
-- 80% matches the portfolio guard's max_gross_exposure default.
UPDATE risk_parameters SET max_portfolio_risk_percent = 80.0 WHERE max_portfolio_risk_percent = 10.0;

-- Lower min_confidence_threshold from 0.70 to 0.55.
-- The agent already filters by its own MIN_CONFIDENCE (default 0.60).
-- Having the risk manager re-check at 0.70 is redundant and stricter,
-- blocking signals the agent intentionally let through.
UPDATE risk_parameters SET min_confidence_threshold = 0.55 WHERE min_confidence_threshold = 0.70;
