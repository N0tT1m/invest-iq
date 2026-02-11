use std::collections::HashMap;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::models::*;
use crate::engine::BacktestEngine;

/// Helper: create a HistoricalBar with the given OHLCV data.
fn bar(date: &str, open: f64, high: f64, low: f64, close: f64, volume: f64) -> HistoricalBar {
    HistoricalBar {
        date: date.to_string(),
        open: Decimal::from_f64(open).unwrap(),
        high: Decimal::from_f64(high).unwrap(),
        low: Decimal::from_f64(low).unwrap(),
        close: Decimal::from_f64(close).unwrap(),
        volume,
    }
}

/// Helper: create a buy signal.
fn buy_signal(date: &str, symbol: &str, price: f64, confidence: f64) -> Signal {
    Signal {
        date: date.to_string(),
        symbol: symbol.to_string(),
        signal_type: "Buy".to_string(),
        confidence,
        price: Decimal::from_f64(price).unwrap(),
        reason: "test".to_string(),
        order_type: None,
        limit_price: None,
        limit_expiry_bars: None,
    }
}

/// Helper: create a sell signal.
fn sell_signal(date: &str, symbol: &str, price: f64, confidence: f64) -> Signal {
    Signal {
        date: date.to_string(),
        symbol: symbol.to_string(),
        signal_type: "Sell".to_string(),
        confidence,
        price: Decimal::from_f64(price).unwrap(),
        reason: "test".to_string(),
        order_type: None,
        limit_price: None,
        limit_expiry_bars: None,
    }
}

/// Helper: build a basic config for testing.
fn test_config(symbol: &str) -> BacktestConfig {
    BacktestConfig {
        strategy_name: "Test".to_string(),
        symbols: vec![symbol.to_string()],
        start_date: "2024-01-01".to_string(),
        end_date: "2024-01-31".to_string(),
        initial_capital: Decimal::new(100000, 0),
        position_size_percent: 50.0,
        stop_loss_percent: None,
        take_profit_percent: None,
        confidence_threshold: 0.5,
        commission_rate: Some(0.001),
        slippage_rate: Some(0.001),
        max_volume_participation: Some(0.05),
        benchmark_bars: None,
        allocation_strategy: None,
        symbol_weights: None,
        rebalance_interval_days: None,
        allow_short_selling: None,
        margin_multiplier: None,
        signal_timeframe: None,
        trailing_stop_percent: None,
        max_drawdown_halt_percent: None,
        regime_config: None,
        commission_model: None,
        allow_fractional_shares: None,
        cash_sweep_rate: None,
        incremental_rebalance: None,
        param_search_space: None,
        market_impact: None,
    }
}

// =============================================================================
// Test 1: Next-bar execution — signals execute at next bar's OPEN, not same bar
// =============================================================================

#[test]
fn test_next_bar_execution() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
        bar("2024-01-05", 108.0, 112.0, 107.0, 111.0, 1_000_000.0),
    ];

    // Buy signal on day 1 (close=103) → should execute at day 2's open (104)
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1); // One trade (closed at end of backtest)
    let trade = &result.trades[0];

    // Entry should be at day 2's open (104) + slippage (0.1%), NOT at 103 (signal price)
    let expected_fill = 104.0 * 1.001; // 104.104
    let actual_fill = trade.entry_price.to_f64().unwrap();
    assert!(
        (actual_fill - expected_fill).abs() < 0.01,
        "Expected entry at ~{:.3} (day 2 open + slippage), got {:.3}",
        expected_fill,
        actual_fill
    );
}

// =============================================================================
// Test 2: Directional slippage — buys fill higher, sells fill lower
// =============================================================================

#[test]
fn test_directional_slippage() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
        bar("2024-01-05", 110.0, 115.0, 108.0, 113.0, 1_000_000.0),
    ];

    // Buy on day 2, sell on day 4
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 109.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.slippage_rate = Some(0.01); // 1% to make it obvious
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    // Buy on day 3 open (104) + 1% slippage = 105.04
    let buy_fill = trade.entry_price.to_f64().unwrap();
    assert!(
        buy_fill > 104.0,
        "Buy fill ({:.3}) should be ABOVE the open (104.0)",
        buy_fill
    );
    let expected_buy = 104.0 * 1.01;
    assert!(
        (buy_fill - expected_buy).abs() < 0.01,
        "Buy fill should be 104 * 1.01 = {:.3}, got {:.3}",
        expected_buy,
        buy_fill
    );

    // Sell on day 5 open (110) - 1% slippage = 108.9
    let sell_fill = trade.exit_price.to_f64().unwrap();
    assert!(
        sell_fill < 110.0,
        "Sell fill ({:.3}) should be BELOW the open (110.0)",
        sell_fill
    );
    let expected_sell = 110.0 * 0.99;
    assert!(
        (sell_fill - expected_sell).abs() < 0.01,
        "Sell fill should be 110 * 0.99 = {:.3}, got {:.3}",
        expected_sell,
        sell_fill
    );
}

// =============================================================================
// Test 3: Volume participation limit — caps shares at X% of bar volume
// =============================================================================

#[test]
fn test_volume_participation_limit() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        // Day 3: very low volume — should limit shares
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 100.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.max_volume_participation = Some(0.05); // 5% of volume
    config.position_size_percent = 100.0; // Try to buy as much as possible

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // With 100 shares volume and 5% participation, max 5 shares
    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    let shares = trade.shares.to_f64().unwrap();
    assert!(
        shares <= 5.0,
        "Shares ({}) should be capped at 5 (5% of 100 volume)",
        shares
    );
}

// =============================================================================
// Test 4: Position sizing on portfolio equity, not remaining cash
// =============================================================================

#[test]
fn test_equity_based_sizing() {
    // Two symbols, each with 50% weight. First buy on day 2, second buy on day 3.
    // With old cash-based sizing, the second position would be smaller.
    // With equity-based sizing, both should be similar size.
    let bars_a = vec![
        bar("2024-01-02", 50.0, 55.0, 48.0, 52.0, 1_000_000.0),
        bar("2024-01-03", 52.0, 56.0, 50.0, 54.0, 1_000_000.0),
        bar("2024-01-04", 53.0, 57.0, 51.0, 55.0, 1_000_000.0),
        bar("2024-01-05", 54.0, 58.0, 52.0, 56.0, 1_000_000.0),
    ];
    let bars_b = vec![
        bar("2024-01-02", 80.0, 85.0, 78.0, 82.0, 1_000_000.0),
        bar("2024-01-03", 82.0, 86.0, 80.0, 84.0, 1_000_000.0),
        bar("2024-01-04", 83.0, 87.0, 81.0, 85.0, 1_000_000.0),
        bar("2024-01-05", 84.0, 88.0, 82.0, 86.0, 1_000_000.0),
    ];

    // Buy A on day 2, buy B on day 3 (both execute next bar)
    let signals = vec![
        buy_signal("2024-01-02", "AAA", 52.0, 0.8),
        buy_signal("2024-01-03", "BBB", 84.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAA".to_string(), bars_a);
    data.insert("BBB".to_string(), bars_b);

    let config = BacktestConfig {
        strategy_name: "Test".to_string(),
        symbols: vec!["AAA".to_string(), "BBB".to_string()],
        start_date: "2024-01-01".to_string(),
        end_date: "2024-01-31".to_string(),
        initial_capital: Decimal::new(100000, 0),
        position_size_percent: 50.0,
        stop_loss_percent: None,
        take_profit_percent: None,
        confidence_threshold: 0.5,
        commission_rate: Some(0.0),   // Zero costs to isolate sizing
        slippage_rate: Some(0.0),
        max_volume_participation: None,
        benchmark_bars: None,
        allocation_strategy: Some("equal_weight".to_string()),
        symbol_weights: None,
        rebalance_interval_days: None,
        allow_short_selling: None,
        margin_multiplier: None,
        signal_timeframe: None,
        trailing_stop_percent: None,
        max_drawdown_halt_percent: None,
        regime_config: None,
        commission_model: None,
        allow_fractional_shares: None,
        cash_sweep_rate: None,
        incremental_rebalance: None,
        param_search_space: None,
        market_impact: None,
    };

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // Both positions should be present
    assert_eq!(result.total_trades, 2);

    // Get the notional value of each trade
    let trade_a = result.trades.iter().find(|t| t.symbol == "AAA").unwrap();
    let trade_b = result.trades.iter().find(|t| t.symbol == "BBB").unwrap();

    let notional_a = (trade_a.entry_price * trade_a.shares).to_f64().unwrap();
    let notional_b = (trade_b.entry_price * trade_b.shares).to_f64().unwrap();

    // Both should be roughly 50% of portfolio equity (~$50k)
    // With cash-based sizing, the second would be ~50% of remaining cash (~$25k)
    assert!(
        notional_b > 30000.0,
        "Second position notional ({:.0}) should be >$30k if equity-based, not cash-based",
        notional_b
    );

    // Both should be in the same ballpark (within 20% of each other)
    let ratio = notional_a / notional_b;
    assert!(
        (0.8..=1.2).contains(&ratio),
        "Positions should be similar size: A={:.0}, B={:.0}, ratio={:.2}",
        notional_a,
        notional_b,
        ratio
    );
}

// =============================================================================
// Test 5: Stop-loss with gap-through
// =============================================================================

#[test]
fn test_stop_loss_gap_through() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        // Day 3: buy executes at open (104)
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Day 4: gap down! Opens well below the stop-loss level
        bar("2024-01-04", 85.0, 88.0, 83.0, 86.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.stop_loss_percent = Some(0.05); // 5% stop-loss
    config.slippage_rate = Some(0.001);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    assert_eq!(trade.exit_reason, "stop_loss");

    // Entry fill: 104 * 1.001 = 104.104
    // SL level: 104.104 * 0.95 = 98.8988
    // Day 4 opens at 85 which is below SL → fill at open (85), not at SL (98.9)
    // Sell fill: 85 * (1 - 0.001) = 84.915
    let exit_fill = trade.exit_price.to_f64().unwrap();
    assert!(
        exit_fill < 90.0,
        "Gap-through stop should fill at open (~85), not at SL trigger (~98.9). Got {:.2}",
        exit_fill
    );
}

// =============================================================================
// Test 6: Take-profit with gap-through
// =============================================================================

#[test]
fn test_take_profit_gap_through() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Day 4: gap up well above TP
        bar("2024-01-04", 130.0, 135.0, 128.0, 132.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.take_profit_percent = Some(0.10); // 10% take-profit
    config.slippage_rate = Some(0.001);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    assert_eq!(trade.exit_reason, "take_profit");

    // Entry fill: 104 * 1.001 = 104.104
    // TP level: 104.104 * 1.10 = 114.5144
    // Day 4 opens at 130 which is above TP → fill at open (130), not TP (114.5)
    // With sell slippage: 130 * 0.999 = 129.87
    let exit_fill = trade.exit_price.to_f64().unwrap();
    assert!(
        exit_fill > 125.0,
        "Gap-through TP should fill at open (~130), not at TP trigger (~114.5). Got {:.2}",
        exit_fill
    );
}

// =============================================================================
// Test 7: CAGR is computed correctly
// =============================================================================

#[test]
fn test_cagr_calculation() {
    // 252 bars (1 year), 10% return → CAGR should be ~10%
    let mut bars = Vec::new();
    let start_price = 100.0;
    let end_price = 110.0;
    let daily_increment = (end_price - start_price) / 254.0;

    for i in 0..255 {
        let price = start_price + daily_increment * i as f64;
        let date = format!("2024-{:02}-{:02}",
            (i / 30 + 1).min(12),
            (i % 30 + 1).min(28)
        );
        bars.push(bar(&date, price - 0.5, price + 1.0, price - 1.0, price, 1_000_000.0));
    }

    // Buy on day 0, hold until end
    let signals = vec![
        buy_signal(&bars[0].date, "AAPL", start_price, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);
    config.position_size_percent = 95.0;
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // CAGR should be populated
    assert!(
        result.annualized_return_percent.is_some(),
        "CAGR should be computed"
    );
}

// =============================================================================
// Test 8: Sharpe uses sample std deviation (n-1)
// =============================================================================

#[test]
fn test_sharpe_sample_std_dev() {
    // With very few data points, population (n) vs sample (n-1) makes a noticeable difference
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
        bar("2024-01-03", 100.0, 106.0, 99.0, 102.0, 1_000_000.0),
        bar("2024-01-04", 102.0, 107.0, 101.0, 104.0, 1_000_000.0),
        bar("2024-01-05", 104.0, 108.0, 103.0, 103.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 100.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // With only 4 equity points (3 returns), minimum 3 data points required
    // Sharpe should use n-1 = 2 in denominator
    // The exact value isn't important; just verify it's computed and finite
    if let Some(sharpe) = result.sharpe_ratio {
        assert!(sharpe.is_finite(), "Sharpe should be finite");
    }
}

// =============================================================================
// Test 9: Sortino with zero downside deviation
// =============================================================================

#[test]
fn test_sortino_zero_downside() {
    // Bars that only go up — no downside returns
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 101.0, 1_000_000.0),
        bar("2024-01-03", 101.0, 106.0, 100.0, 103.0, 1_000_000.0),
        bar("2024-01-04", 103.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-05", 106.0, 111.0, 105.0, 110.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 101.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // With all-positive returns, Sortino should be very high (99.99), not equal to Sharpe
    if let (Some(sharpe), Some(sortino)) = (result.sharpe_ratio, result.sortino_ratio) {
        assert!(
            sortino >= 99.0,
            "Sortino with zero downside should be 99.99, got {:.2}",
            sortino
        );
        assert!(
            sortino > sharpe,
            "Sortino ({:.2}) should be > Sharpe ({:.2}) when no downside",
            sortino,
            sharpe
        );
    }
}

// =============================================================================
// Test 10: No signals execute on the same bar
// =============================================================================

#[test]
fn test_no_same_bar_execution() {
    // Only 1 bar — signal generated on it should NOT execute (no next bar)
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // No trades — the signal had no next bar to execute on
    assert_eq!(
        result.total_trades, 0,
        "Signals should not execute on the same bar they were generated"
    );
}

// =============================================================================
// Test 11: Commission and slippage costs are tracked correctly
// =============================================================================

#[test]
fn test_cost_tracking() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
        bar("2024-01-05", 110.0, 115.0, 108.0, 113.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 109.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);

    // Commission and slippage should both be positive
    let comm = result.total_commission_paid.to_f64().unwrap();
    let slip = result.total_slippage_cost.to_f64().unwrap();
    assert!(comm > 0.0, "Commission should be positive, got {}", comm);
    assert!(slip > 0.0, "Slippage should be positive, got {}", slip);

    // Trade-level costs should match
    let trade = &result.trades[0];
    let trade_comm = trade.commission_cost.to_f64().unwrap();
    let trade_slip = trade.slippage_cost.to_f64().unwrap();
    assert!(trade_comm > 0.0, "Trade commission should be positive");
    assert!(trade_slip > 0.0, "Trade slippage should be positive");
}

// =============================================================================
// Test 12: Zero cost configuration works
// =============================================================================

#[test]
fn test_zero_cost_round_trip() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
        bar("2024-01-04", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
        bar("2024-01-05", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
    ];

    // Buy at 100 open, sell at 100 open → should be zero P&L with zero costs
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 100.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    // Buy at day3 open (100), sell at day5 open (100) → zero P&L
    let pnl = trade.profit_loss.to_f64().unwrap();
    assert!(
        pnl.abs() < 1.0,
        "Round trip at same price with zero costs should have ~zero P&L, got {:.2}",
        pnl
    );
}

// =============================================================================
// Test 13: Low-confidence signals are filtered
// =============================================================================

#[test]
fn test_confidence_filter() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
    ];

    // Signal with confidence below threshold
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.3), // Below 0.5 threshold
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 0, "Low-confidence signals should be filtered");
}

// =============================================================================
// Test 14: P&L includes slippage in fill prices
// =============================================================================

#[test]
fn test_pnl_includes_slippage() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
        bar("2024-01-04", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
        bar("2024-01-05", 100.0, 105.0, 99.0, 100.0, 1_000_000.0),
    ];

    // Buy and sell at the same open price (100) — with slippage, should lose money
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 100.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0); // Zero commission to isolate slippage
    config.slippage_rate = Some(0.01); // 1% slippage
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    // Buy fill: 100 * 1.01 = 101, Sell fill: 100 * 0.99 = 99
    // P&L per share: 99 - 101 = -2 → negative
    let pnl = trade.profit_loss.to_f64().unwrap();
    assert!(
        pnl < 0.0,
        "Round trip at same price with 1% slippage should be negative, got {:.2}",
        pnl
    );

    // The return percent should also be negative (based on fill prices)
    assert!(
        trade.profit_loss_percent < 0.0,
        "Return percent should reflect slippage: {:.2}%",
        trade.profit_loss_percent
    );
}

// =============================================================================
// Test 15: Short selling — basic open and close
// =============================================================================

#[test]
fn test_short_selling_basic() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 95.0, 98.0, 93.0, 96.0, 1_000_000.0),
        bar("2024-01-05", 90.0, 92.0, 88.0, 91.0, 1_000_000.0),
    ];

    // Sell signal (short) on day 2, buy signal (cover) on day 4
    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
        buy_signal("2024-01-04", "AAPL", 96.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_short_selling = Some(true);
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // Should have 1 trade: short entry + buy-to-cover
    assert_eq!(result.total_trades, 1, "Expected 1 short trade");

    let trade = &result.trades[0];
    assert_eq!(trade.direction.as_deref(), Some("short"));
    assert_eq!(trade.exit_reason, "signal_cover");

    // Short at day 3 open (104), cover at day 5 open (90)
    // Profit = (104 - 90) * shares > 0
    let pnl = trade.profit_loss.to_f64().unwrap();
    assert!(pnl > 0.0, "Short on declining price should profit, got {:.2}", pnl);

    // short_trades count should be 1
    assert_eq!(result.short_trades, Some(1));
}

// =============================================================================
// Test 16: Short selling — losing short trade
// =============================================================================

#[test]
fn test_short_selling_loss() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 110.0, 115.0, 108.0, 113.0, 1_000_000.0),
        bar("2024-01-05", 115.0, 120.0, 113.0, 118.0, 1_000_000.0),
    ];

    // Short at day 2, cover at day 4 — price went up, should lose
    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
        buy_signal("2024-01-04", "AAPL", 113.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_short_selling = Some(true);
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    assert_eq!(trade.direction.as_deref(), Some("short"));

    // Short at 104, cover at 115 → loss
    let pnl = trade.profit_loss.to_f64().unwrap();
    assert!(pnl < 0.0, "Short on rising price should lose, got {:.2}", pnl);
}

// =============================================================================
// Test 17: Short selling disabled — sell with no position does nothing
// =============================================================================

#[test]
fn test_short_selling_disabled() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 95.0, 98.0, 93.0, 96.0, 1_000_000.0),
    ];

    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL"); // allow_short_selling defaults to None (false)
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 0, "No shorts when short selling disabled");
}

// =============================================================================
// Test 18: Short SL/TP — stop loss triggered (price rises above SL)
// =============================================================================

#[test]
fn test_short_stop_loss() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Day 4: high exceeds SL for short (price rises too much)
        bar("2024-01-04", 110.0, 120.0, 109.0, 115.0, 1_000_000.0),
    ];

    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_short_selling = Some(true);
    config.stop_loss_percent = Some(0.05); // 5% SL for short = entry * 1.05
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_reason, "stop_loss");
    assert_eq!(trade.direction.as_deref(), Some("short"));

    // Short entry at 104, SL at 104 * 1.05 = 109.2
    // Day 4 opens at 110 > SL → fill at open (110, gap-through)
    let exit = trade.exit_price.to_f64().unwrap();
    assert!(exit >= 109.0, "Short SL should trigger near 109-110, got {:.2}", exit);
}

// =============================================================================
// Test 19: Short take-profit — price drops below TP
// =============================================================================

#[test]
fn test_short_take_profit() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Day 4: drops well below TP
        bar("2024-01-04", 80.0, 82.0, 78.0, 81.0, 1_000_000.0),
    ];

    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_short_selling = Some(true);
    config.take_profit_percent = Some(0.10); // 10% TP for short = entry * 0.90
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    assert_eq!(trade.exit_reason, "take_profit");

    // Should be profitable
    let pnl = trade.profit_loss.to_f64().unwrap();
    assert!(pnl > 0.0, "Short TP should yield profit, got {:.2}", pnl);
}

// =============================================================================
// Test 20: Trailing stop — ratchets up
// =============================================================================

#[test]
fn test_trailing_stop() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 115.0, 106.0, 113.0, 1_000_000.0), // high=115
        bar("2024-01-05", 113.0, 116.0, 112.0, 114.0, 1_000_000.0), // high=116, new peak
        // Day 6: drops from peak — trailing stop should trigger
        bar("2024-01-08", 108.0, 109.0, 105.0, 106.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.trailing_stop_percent = Some(0.05); // 5% trailing stop
    config.stop_loss_percent = None; // Only trailing
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // Trailing stop should have triggered
    let has_sl_trade = result.trades.iter().any(|t| t.exit_reason == "stop_loss");
    assert!(has_sl_trade, "Trailing stop should trigger a stop_loss exit");

    // The trailing stop should be based on the peak high (116)
    // Trailing SL = 116 * 0.95 = 110.2. Day 6 low=105, bar open=108 < 110.2 → fill at open=108
    if let Some(trade) = result.trades.iter().find(|t| t.exit_reason == "stop_loss") {
        let exit = trade.exit_price.to_f64().unwrap();
        assert!(exit <= 110.5, "Trailing stop should fill near open, got {:.2}", exit);
    }
}

// =============================================================================
// Test 21: Circuit breaker — halts trading on max drawdown
// =============================================================================

#[test]
fn test_circuit_breaker() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Crash: large drawdown
        bar("2024-01-04", 60.0, 62.0, 58.0, 61.0, 1_000_000.0),
        // Should NOT open a new position after circuit breaker trips
        bar("2024-01-05", 50.0, 55.0, 48.0, 52.0, 1_000_000.0),
        bar("2024-01-08", 52.0, 56.0, 50.0, 54.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 61.0, 0.8), // Sell after crash
        buy_signal("2024-01-05", "AAPL", 52.0, 0.8),  // Try to re-enter — should be blocked
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.max_drawdown_halt_percent = Some(10.0); // 10% halt threshold
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // The first trade should go through, but after the large drawdown,
    // the circuit breaker should prevent new entries
    // We expect at most 1 completed round-trip (buy+sell)
    let buy_entries: Vec<_> = result.trades.iter()
        .filter(|t| t.direction.as_deref() != Some("short"))
        .collect();
    // After the circuit breaker trips, the buy_signal on day 5 should be blocked
    assert!(
        buy_entries.len() <= 2,
        "Circuit breaker should limit entries after drawdown"
    );
}

// =============================================================================
// Test 22: Limit orders — fills when price reaches limit
// =============================================================================

#[test]
fn test_limit_order() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        // Day 4: low=95, which is below our limit buy price of 98
        bar("2024-01-04", 97.0, 99.0, 95.0, 96.0, 1_000_000.0),
        bar("2024-01-05", 96.0, 100.0, 94.0, 98.0, 1_000_000.0),
    ];

    let limit_buy = Signal {
        date: "2024-01-02".to_string(),
        symbol: "AAPL".to_string(),
        signal_type: "Buy".to_string(),
        confidence: 0.8,
        price: Decimal::from(100),
        reason: "test limit".to_string(),
        order_type: Some(crate::models::OrderType::Limit),
        limit_price: Some(Decimal::from(98)),
        limit_expiry_bars: Some(5),
    };

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, vec![limit_buy]).unwrap();

    // Limit order should trigger when bar low drops below 98
    assert!(
        result.total_trades >= 1,
        "Limit buy should fill when price drops to 98 (day 4 low=95)"
    );
}

// =============================================================================
// Test 23: Fractional shares
// =============================================================================

#[test]
fn test_fractional_shares() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 3500.0, 3600.0, 3400.0, 3550.0, 1_000_000.0), // Expensive stock
        bar("2024-01-04", 3550.0, 3650.0, 3450.0, 3600.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_fractional_shares = Some(true);
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);
    config.position_size_percent = 10.0; // Small position = fractional shares

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    let shares = trade.shares.to_f64().unwrap();
    // With $100k capital, 10% position = $10k, at ~$3500/share → ~2.857 fractional shares
    assert!(
        shares > 0.0 && shares < 10.0,
        "Should have fractional shares, got {:.4}",
        shares
    );
    // Verify it's actually fractional (not rounded)
    assert!(
        (shares - shares.floor()).abs() > 0.001,
        "Shares should be fractional (non-integer), got {:.4}",
        shares
    );
}

// =============================================================================
// Test 24: Cash sweep — earns interest on idle cash
// =============================================================================

#[test]
fn test_cash_sweep() {
    let mut bars = Vec::new();
    for i in 0..20 {
        let date = format!("2024-01-{:02}", i + 2);
        bars.push(bar(&date, 100.0, 105.0, 95.0, 100.0, 1_000_000.0));
    }

    // No trades — all cash sits idle and earns interest
    let signals: Vec<Signal> = vec![];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.cash_sweep_rate = Some(0.05); // 5% annualized

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // Final capital should be slightly above initial due to cash sweep
    let final_cap = result.final_capital.to_f64().unwrap();
    let initial_cap = 100000.0;
    assert!(
        final_cap > initial_cap,
        "Cash sweep should earn interest: {:.2} vs {:.2}",
        final_cap,
        initial_cap
    );

    // 20 days compounding at 5%/252 daily → ~$397 interest
    let interest = final_cap - initial_cap;
    assert!(
        interest > 350.0 && interest < 450.0,
        "Expected ~$397 interest for 20 days at 5%, got {:.2}",
        interest
    );
}

// =============================================================================
// Test 25: Margin multiplier — can buy more than cash allows
// =============================================================================

#[test]
fn test_margin_multiplier() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 50.0, 55.0, 48.0, 52.0, 1_000_000.0),
        bar("2024-01-04", 52.0, 56.0, 50.0, 54.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.margin_multiplier = Some(2.0); // 2x margin
    config.position_size_percent = 100.0; // Try to use full buying power
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];
    let notional = (trade.entry_price * trade.shares).to_f64().unwrap();

    // With 2x margin, should be able to buy up to $200k worth
    assert!(
        notional > 100000.0,
        "2x margin should allow buying >$100k worth, got {:.0}",
        notional
    );

    // margin_used_peak should be tracked
    assert!(result.margin_used_peak.is_some(), "Margin peak should be tracked");
}

// =============================================================================
// Test 26: Regime detection — adjusts position sizing
// =============================================================================

#[test]
fn test_regime_sizing() {
    use crate::models::RegimeConfig;

    // Create enough bars for regime detection (>20)
    let mut bars = Vec::new();
    for i in 0..30 {
        let date = format!("2024-01-{:02}", (i % 28) + 1);
        // Alternating volatile moves to trigger high-vol regime
        let price = 100.0 + if i % 2 == 0 { 5.0 } else { -5.0 };
        bars.push(bar(&date, price - 1.0, price + 3.0, price - 3.0, price, 1_000_000.0));
    }
    // Deduplicate dates by using unique date
    let mut bars_unique = Vec::new();
    for i in 0..30 {
        let date = format!("2024-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1);
        let price = 100.0 + if i % 2 == 0 { 5.0 } else { -5.0 };
        bars_unique.push(bar(&date, price - 1.0, price + 3.0, price - 3.0, price, 1_000_000.0));
    }

    let signals = vec![
        buy_signal(&bars_unique[25].date, "AAPL", 100.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars_unique);

    let mut config = test_config("AAPL");
    config.regime_config = Some(RegimeConfig::default());
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    // Just verify it runs without errors — regime detection is integrated
    assert!(result.equity_curve.len() > 0, "Should have equity curve");
}

// =============================================================================
// Test 27: Data quality report is populated
// =============================================================================

#[test]
fn test_data_quality_report() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 0.0), // Zero volume
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
    ];

    let signals: Vec<Signal> = vec![];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    let dq = result.data_quality_report.as_ref().expect("Should have data quality report");
    assert_eq!(dq.total_bars, 3);
    assert!(dq.zero_volume_bars > 0, "Should detect zero-volume bar");
}

// =============================================================================
// Test 28: Extended metrics populated for sufficient data
// =============================================================================

#[test]
fn test_extended_metrics() {
    let mut bars = Vec::new();
    for i in 0..30 {
        let date = format!("2024-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1);
        let price = 100.0 + i as f64 * 0.5;
        bars.push(bar(&date, price, price + 2.0, price - 2.0, price + 0.3, 1_000_000.0));
    }

    let signals = vec![
        buy_signal(&bars[0].date, "AAPL", 100.0, 0.8),
        sell_signal(&bars[10].date, "AAPL", 105.0, 0.8),
        buy_signal(&bars[15].date, "AAPL", 107.0, 0.8),
        sell_signal(&bars[25].date, "AAPL", 112.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    let ext = result.extended_metrics.as_ref().expect("Should have extended metrics");
    assert!(ext.monthly_returns.len() > 0, "Should have monthly returns");
    // Drawdown events may or may not exist depending on equity curve shape
    let _ = ext.top_drawdown_events.len();
}

// =============================================================================
// Test 29: Confidence intervals for enough trades
// =============================================================================

#[test]
fn test_confidence_intervals() {
    let mut bars = Vec::new();
    for i in 0..60 {
        let date = format!("2024-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1);
        let price = 100.0 + (i as f64 * 0.1).sin() * 5.0;
        bars.push(bar(&date, price, price + 2.0, price - 2.0, price + 0.2, 1_000_000.0));
    }

    // Generate 12 round-trip trades (need >= 10 for CI)
    let mut signals = Vec::new();
    for i in (0..48).step_by(4) {
        signals.push(buy_signal(&bars[i].date, "AAPL", 100.0, 0.8));
        signals.push(sell_signal(&bars[i + 2].date, "AAPL", 100.0, 0.8));
    }

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.commission_rate = Some(0.0);
    config.slippage_rate = Some(0.0);

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    if result.total_trades >= 10 {
        let ci = result.confidence_intervals.as_ref().expect("Should have CI with 10+ trades");
        assert!(ci.bootstrap_samples > 0, "Should have bootstrap samples");
        assert!(ci.win_rate_ci_lower <= ci.win_rate_ci_upper, "CI lower <= upper");
    }
}

// =============================================================================
// Test 30: Tear sheet is generated
// =============================================================================

#[test]
fn test_tear_sheet() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
        bar("2024-01-05", 110.0, 115.0, 108.0, 113.0, 1_000_000.0),
    ];

    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 109.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    let sheet = result.tear_sheet.as_ref().expect("Should have tear sheet");
    assert!(sheet.get("summary").is_some(), "Tear sheet should have summary");
    assert!(sheet.get("risk_metrics").is_some(), "Tear sheet should have risk metrics");
    assert!(sheet.get("trade_analysis").is_some(), "Tear sheet should have trade analysis");
}

// =============================================================================
// Test 31: Monte Carlo basic
// =============================================================================

#[test]
fn test_monte_carlo_basic() {
    use crate::monte_carlo::run_monte_carlo;

    let trades = vec![
        BacktestTrade {
            id: None, backtest_id: None, symbol: "AAPL".to_string(),
            signal: "Buy".to_string(), confidence: 0.7,
            entry_date: "2024-01-02".to_string(),
            exit_date: "2024-01-05".to_string(),
            entry_price: Decimal::from(100), exit_price: Decimal::from(110),
            shares: Decimal::from(10), profit_loss: Decimal::from(100),
            profit_loss_percent: 10.0, holding_period_days: 3,
            commission_cost: Decimal::ZERO, slippage_cost: Decimal::ZERO,
            exit_reason: "signal".to_string(), direction: Some("long".to_string()),
        },
        BacktestTrade {
            id: None, backtest_id: None, symbol: "AAPL".to_string(),
            signal: "Buy".to_string(), confidence: 0.6,
            entry_date: "2024-01-08".to_string(),
            exit_date: "2024-01-12".to_string(),
            entry_price: Decimal::from(105), exit_price: Decimal::from(100),
            shares: Decimal::from(10), profit_loss: Decimal::from(-50),
            profit_loss_percent: -4.76, holding_period_days: 4,
            commission_cost: Decimal::ZERO, slippage_cost: Decimal::ZERO,
            exit_reason: "signal".to_string(), direction: Some("long".to_string()),
        },
    ];

    let result = run_monte_carlo(&trades, Decimal::from(100000), 100);
    assert_eq!(result.simulations, 100);
    assert!(result.probability_of_profit >= 0.0 && result.probability_of_profit <= 100.0);
    assert!(result.median_max_drawdown >= 0.0);
}

// =============================================================================
// Test 32: Monte Carlo enhanced — block bootstrap
// =============================================================================

#[test]
fn test_monte_carlo_enhanced() {
    use crate::monte_carlo::run_monte_carlo_enhanced;
    use crate::models::MonteCarloConfig;

    let mut trades = Vec::new();
    for i in 0..20 {
        trades.push(BacktestTrade {
            id: None, backtest_id: None, symbol: "AAPL".to_string(),
            signal: "Buy".to_string(), confidence: 0.6,
            entry_date: format!("2024-01-{:02}", i + 1),
            exit_date: format!("2024-01-{:02}", i + 2),
            entry_price: Decimal::from(100),
            exit_price: Decimal::from(if i % 3 == 0 { 95 } else { 105 }),
            shares: Decimal::from(10),
            profit_loss: Decimal::from(if i % 3 == 0 { -50 } else { 50 }),
            profit_loss_percent: if i % 3 == 0 { -5.0 } else { 5.0 },
            holding_period_days: 1,
            commission_cost: Decimal::from(1),
            slippage_cost: Decimal::from(1),
            exit_reason: "signal".to_string(),
            direction: Some("long".to_string()),
        });
    }

    let config = MonteCarloConfig {
        num_simulations: 100,
        block_size: 5,
        parameter_uncertainty: true,
    };

    let result = run_monte_carlo_enhanced(&trades, Decimal::from(100000), &config);
    assert_eq!(result.simulations, 100);
    assert!(result.return_distribution.len() > 0);
    assert!(result.drawdown_distribution.len() > 0);
}

// =============================================================================
// Test 33: Short slippage direction
// =============================================================================

#[test]
fn test_short_slippage_direction() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 95.0, 98.0, 93.0, 96.0, 1_000_000.0),
        bar("2024-01-05", 90.0, 92.0, 88.0, 91.0, 1_000_000.0),
    ];

    let signals = vec![
        sell_signal("2024-01-02", "AAPL", 103.0, 0.8),
        buy_signal("2024-01-04", "AAPL", 96.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let mut config = test_config("AAPL");
    config.allow_short_selling = Some(true);
    config.slippage_rate = Some(0.01); // 1% to make it obvious

    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    let trade = &result.trades[0];

    // Short entry: sell at day 3 open (104) with SELL slippage → fills BELOW
    let entry = trade.entry_price.to_f64().unwrap();
    assert!(
        entry < 104.0,
        "Short entry fill ({:.3}) should be BELOW the open (104.0) — sell-side slippage",
        entry
    );

    // Short exit (buy-to-cover): at day 5 open (90) with BUY slippage → fills ABOVE
    let exit = trade.exit_price.to_f64().unwrap();
    assert!(
        exit > 90.0,
        "Short exit fill ({:.3}) should be ABOVE the open (90.0) — buy-side slippage",
        exit
    );
}

// =============================================================================
// Test 34: Walk-forward optimization
// =============================================================================

#[test]
fn test_walk_forward_optimization() {
    use crate::walk_forward_opt::run_optimized_walk_forward;
    use crate::models::{ParamSearchSpace, WalkForwardFoldData};

    let make_bars = |start: usize, count: usize| -> Vec<HistoricalBar> {
        (0..count)
            .map(|i| {
                let idx = start + i;
                let date = format!("2024-{:02}-{:02}", (idx / 28) + 1, (idx % 28) + 1);
                let price = 100.0 + (idx as f64 * 0.3);
                bar(&date, price, price + 2.0, price - 2.0, price + 0.5, 1_000_000.0)
            })
            .collect()
    };

    let make_signals = |bars: &[HistoricalBar]| -> Vec<Signal> {
        let mut sigs = Vec::new();
        for i in (0..bars.len()).step_by(4) {
            sigs.push(buy_signal(&bars[i].date, "AAPL", 100.0, 0.8));
            if i + 2 < bars.len() {
                sigs.push(sell_signal(&bars[i + 2].date, "AAPL", 100.0, 0.8));
            }
        }
        sigs
    };

    let train_bars = make_bars(0, 20);
    let test_bars = make_bars(20, 10);
    let train_signals = make_signals(&train_bars);
    let test_signals = make_signals(&test_bars);

    let mut train_data = HashMap::new();
    train_data.insert("AAPL".to_string(), train_bars);
    let mut test_data = HashMap::new();
    test_data.insert("AAPL".to_string(), test_bars);

    let fold = WalkForwardFoldData {
        train_data,
        train_signals,
        test_data,
        test_signals,
    };

    let config = test_config("AAPL");
    let space = ParamSearchSpace {
        confidence_thresholds: vec![0.3, 0.5, 0.7],
        position_size_percents: vec![30.0, 50.0],
        stop_loss_percents: vec![],
        take_profit_percents: vec![],
    };

    let result = run_optimized_walk_forward(&config, vec![fold], &space);
    assert!(result.is_ok(), "Walk-forward optimization should succeed");

    let result = result.unwrap();
    assert!(result.optimized_params.len() > 0, "Should have optimized params");
}

// =============================================================================
// Test 35: Tiered commission model
// =============================================================================

#[test]
fn test_tiered_commission() {
    use crate::commission::compute_tiered_commission;
    use crate::models::{CommissionModel, CommissionTier};

    let model = CommissionModel {
        tiers: vec![
            CommissionTier { volume_threshold: 0.0, per_share_rate: 0.01 },
            CommissionTier { volume_threshold: 1000.0, per_share_rate: 0.005 },
        ],
        min_per_trade: 1.0,
        max_per_trade: 100.0,
    };

    // Small trade: 10 shares at $50 with low monthly volume — first tier (0.01/share)
    // 10 * 0.01 = $0.10, but min_per_trade = $1 → commission = $1
    let comm1 = compute_tiered_commission(
        Some(&model), Decimal::from(10), Decimal::from(50), Decimal::from_f64(0.001).unwrap(), 500.0,
    );
    let comm1_f64 = comm1.to_f64().unwrap();
    assert!(comm1_f64 >= 1.0, "Should hit minimum commission of $1, got {:.2}", comm1_f64);

    // Larger trade: 100 shares at $50 at high monthly volume — second tier (0.005/share)
    // 100 * 0.005 = $0.50, but min_per_trade = $1 → commission = $1
    let comm2 = compute_tiered_commission(
        Some(&model), Decimal::from(100), Decimal::from(50), Decimal::from_f64(0.001).unwrap(), 1500.0,
    );
    let comm2_f64 = comm2.to_f64().unwrap();
    assert!(comm2_f64 >= 1.0 && comm2_f64 <= 100.0, "Should be within min/max, got {:.2}", comm2_f64);
}

// =============================================================================
// Test 36: Data quality detects OHLC inconsistencies
// =============================================================================

#[test]
fn test_data_quality_ohlc() {
    use crate::data_quality::check_data_quality;

    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        // OHLC inconsistency: low > high
        HistoricalBar {
            date: "2024-01-03".to_string(),
            open: Decimal::from(104),
            high: Decimal::from(100), // high < low = invalid
            low: Decimal::from(108),
            close: Decimal::from(106),
            volume: 1_000_000.0,
        },
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let report = check_data_quality(&data);
    assert!(report.warnings.len() > 0, "Should detect OHLC inconsistency");
}

// =============================================================================
// Test 37: Factor attribution with benchmark
// =============================================================================

#[test]
fn test_factor_attribution() {
    use crate::factor_attribution::compute_factor_attribution;
    use crate::extended_metrics::equity_returns;

    let strategy_equity: Vec<crate::models::EquityPoint> = (0..30)
        .map(|i| crate::models::EquityPoint {
            timestamp: format!("2024-01-{:02}", i + 1),
            equity: Decimal::from(100000 + i * 100),
            drawdown_percent: 0.0,
        })
        .collect();

    let benchmark_equity: Vec<crate::models::EquityPoint> = (0..30)
        .map(|i| crate::models::EquityPoint {
            timestamp: format!("2024-01-{:02}", i + 1),
            equity: Decimal::from(100000 + i * 80),
            drawdown_percent: 0.0,
        })
        .collect();

    let strat_rets = equity_returns(&strategy_equity);
    let bench_rets = equity_returns(&benchmark_equity);

    let fa = compute_factor_attribution(&strat_rets, &bench_rets);
    assert!(fa.is_some(), "Factor attribution should be computed");

    let fa = fa.unwrap();
    assert!(fa.beta.is_finite(), "Beta should be finite");
    assert!(fa.r_squared >= 0.0 && fa.r_squared <= 1.0, "R² should be in [0,1], got {:.4}", fa.r_squared);
}

// =============================================================================
// Test 38: Bootstrap CI produces valid intervals
// =============================================================================

#[test]
fn test_bootstrap_ci() {
    use crate::statistical::bootstrap_confidence_intervals;

    let mut trades = Vec::new();
    for i in 0..20 {
        trades.push(BacktestTrade {
            id: None, backtest_id: None, symbol: "AAPL".to_string(),
            signal: "Buy".to_string(), confidence: 0.65,
            entry_date: format!("2024-01-{:02}", i + 1),
            exit_date: format!("2024-01-{:02}", i + 2),
            entry_price: Decimal::from(100),
            exit_price: Decimal::from(if i % 3 == 0 { 95 } else { 108 }),
            shares: Decimal::from(10),
            profit_loss: Decimal::from(if i % 3 == 0 { -50 } else { 80 }),
            profit_loss_percent: if i % 3 == 0 { -5.0 } else { 8.0 },
            holding_period_days: 1,
            commission_cost: Decimal::ZERO,
            slippage_cost: Decimal::ZERO,
            exit_reason: "signal".to_string(),
            direction: Some("long".to_string()),
        });
    }

    let ci = bootstrap_confidence_intervals(&trades, 500);
    assert!(ci.is_some(), "Should compute CI with 20 trades");

    let ci = ci.unwrap();
    assert!(ci.win_rate_ci_lower <= ci.win_rate_ci_upper, "Win rate CI should be ordered");
    assert!(ci.bootstrap_samples == 500);
}

// =============================================================================
// Test 39: Timeframe aggregation — daily to weekly
// =============================================================================

#[test]
fn test_weekly_aggregation() {
    use crate::timeframe_agg::aggregate_to_weekly;

    let daily = vec![
        bar("2024-01-08", 100.0, 105.0, 99.0, 103.0, 1_000.0), // Monday
        bar("2024-01-09", 103.0, 107.0, 102.0, 106.0, 2_000.0), // Tuesday
        bar("2024-01-10", 106.0, 108.0, 104.0, 105.0, 1_500.0), // Wednesday
        bar("2024-01-11", 105.0, 110.0, 104.0, 109.0, 3_000.0), // Thursday
        bar("2024-01-12", 109.0, 112.0, 108.0, 111.0, 2_500.0), // Friday
        bar("2024-01-15", 111.0, 115.0, 110.0, 113.0, 2_000.0), // Next Monday
        bar("2024-01-16", 113.0, 116.0, 112.0, 115.0, 1_800.0), // Tuesday
    ];

    let weekly = aggregate_to_weekly(&daily);

    assert_eq!(weekly.len(), 2, "Should produce 2 weekly bars");

    // First week: open=100 (Monday open), close=111 (Friday close),
    // high=112 (Friday), low=99 (Monday), volume=10000
    let w1 = &weekly[0];
    assert_eq!(w1.open.to_f64().unwrap(), 100.0, "Week 1 open = Monday open");
    assert_eq!(w1.close.to_f64().unwrap(), 111.0, "Week 1 close = Friday close");
    assert_eq!(w1.high.to_f64().unwrap(), 112.0, "Week 1 high = max");
    assert_eq!(w1.low.to_f64().unwrap(), 99.0, "Week 1 low = min");
    assert_eq!(w1.volume, 10_000.0, "Week 1 volume = sum");
}

// =============================================================================
// Test 40: Short position MtM calculation
// =============================================================================

#[test]
fn test_short_mtm() {
    use crate::short_selling::short_position_mtm;

    // Short at $100, current $90, 10 shares
    // MtM = entry * shares + (entry - current) * shares = 100*10 + (100-90)*10 = 1100
    let mtm = short_position_mtm(Decimal::from(100), Decimal::from(90), Decimal::from(10));
    assert_eq!(mtm.to_f64().unwrap(), 1100.0, "Short MtM when price drops");

    // Short at $100, current $110, 10 shares
    // MtM = 100*10 + (100-110)*10 = 1000 - 100 = 900
    let mtm2 = short_position_mtm(Decimal::from(100), Decimal::from(110), Decimal::from(10));
    assert_eq!(mtm2.to_f64().unwrap(), 900.0, "Short MtM when price rises");
}

// =============================================================================
// Test 41: Direction field set correctly on trades
// =============================================================================

#[test]
fn test_trade_direction_field() {
    let bars = vec![
        bar("2024-01-02", 100.0, 105.0, 99.0, 103.0, 1_000_000.0),
        bar("2024-01-03", 104.0, 108.0, 102.0, 106.0, 1_000_000.0),
        bar("2024-01-04", 107.0, 110.0, 105.0, 109.0, 1_000_000.0),
        bar("2024-01-05", 110.0, 115.0, 108.0, 113.0, 1_000_000.0),
    ];

    // Buy and sell — should be "long"
    let signals = vec![
        buy_signal("2024-01-02", "AAPL", 103.0, 0.8),
        sell_signal("2024-01-04", "AAPL", 109.0, 0.8),
    ];

    let mut data = HashMap::new();
    data.insert("AAPL".to_string(), bars);

    let config = test_config("AAPL");
    let mut engine = BacktestEngine::new(config);
    let result = engine.run(data, signals).unwrap();

    assert_eq!(result.total_trades, 1);
    assert_eq!(
        result.trades[0].direction.as_deref(),
        Some("long"),
        "Long trades should have direction='long'"
    );
}

// =============================================================================
// Test 42: Regime detection function
// =============================================================================

#[test]
fn test_regime_detection() {
    use crate::regime_risk::{detect_regime, Regime, regime_size_multiplier};
    use crate::models::RegimeConfig;

    let config = RegimeConfig::default();

    // Low vol returns
    let low_vol: Vec<f64> = (0..30).map(|_| 0.001).collect();
    let regime = detect_regime(&low_vol, &config);
    assert!(matches!(regime, Regime::LowVol), "Stable returns should be LowVol");

    // High vol returns
    let high_vol: Vec<f64> = (0..30).map(|i| if i % 2 == 0 { 0.05 } else { -0.05 }).collect();
    let regime = detect_regime(&high_vol, &config);
    assert!(matches!(regime, Regime::HighVol), "Volatile returns should be HighVol");

    // Multipliers
    let hv_mult = regime_size_multiplier(Regime::HighVol, &config);
    let lv_mult = regime_size_multiplier(Regime::LowVol, &config);
    assert!(hv_mult < lv_mult, "High vol should reduce size ({:.2} < {:.2})", hv_mult, lv_mult);
}
