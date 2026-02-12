#[cfg(test)]
mod risk_manager_tests {
    use crate::manager::RiskManager;

    async fn setup_test_db() -> RiskManager {
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");

        // Create required tables
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS risk_parameters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                max_risk_per_trade_percent REAL NOT NULL DEFAULT 2.0,
                max_portfolio_risk_percent REAL NOT NULL DEFAULT 80.0,
                max_position_size_percent REAL NOT NULL DEFAULT 20.0,
                default_stop_loss_percent REAL NOT NULL DEFAULT 5.0,
                default_take_profit_percent REAL NOT NULL DEFAULT 10.0,
                trailing_stop_enabled INTEGER NOT NULL DEFAULT 0,
                trailing_stop_percent REAL NOT NULL DEFAULT 3.0,
                min_confidence_threshold REAL NOT NULL DEFAULT 0.55,
                min_win_rate_threshold REAL NOT NULL DEFAULT 0.55,
                daily_loss_limit_percent REAL NOT NULL DEFAULT 5.0,
                max_consecutive_losses INTEGER NOT NULL DEFAULT 3,
                account_drawdown_limit_percent REAL NOT NULL DEFAULT 10.0,
                trading_halted INTEGER NOT NULL DEFAULT 0,
                halt_reason TEXT,
                halted_at TEXT,
                updated_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS portfolio_peak (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                peak_value REAL NOT NULL,
                peak_date TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                action TEXT NOT NULL,
                shares REAL NOT NULL,
                price REAL NOT NULL,
                timestamp TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS active_risk_positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                shares REAL NOT NULL,
                entry_price REAL NOT NULL,
                entry_date TEXT NOT NULL,
                stop_loss_price REAL,
                take_profit_price REAL,
                trailing_stop_enabled INTEGER NOT NULL DEFAULT 0,
                trailing_stop_percent REAL,
                max_price_seen REAL,
                risk_amount REAL,
                position_size_percent REAL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT DEFAULT (datetime('now')),
                closed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        RiskManager::new(pool)
    }

    #[tokio::test]
    async fn circuit_breaker_halted() {
        let rm = setup_test_db().await;

        // Manually halt trading
        rm.set_trading_halt(true, Some("Test halt")).await.unwrap();

        // Check circuit breakers
        let check = rm.check_circuit_breakers(100_000.0, 0.0).await.unwrap();
        assert!(!check.can_trade);
        assert!(check
            .breakers_triggered
            .contains(&"manual_halt".to_string()));
    }

    #[tokio::test]
    async fn low_confidence_rejected() {
        let rm = setup_test_db().await;

        // Default min_confidence_threshold is 0.70
        // Try with low confidence (0.3 = 30%)
        let check = rm.check_trade_risk(0.3, 100_000.0, 0.0, 0).await.unwrap();
        assert!(!check.can_trade);
        assert!(check.reason.contains("Confidence"));

        // Try with high confidence (0.9 = 90%)
        let check = rm.check_trade_risk(0.9, 100_000.0, 0.0, 0).await.unwrap();
        assert!(check.can_trade);
    }

    #[tokio::test]
    async fn consecutive_losses_counted() {
        let rm = setup_test_db().await;

        // Insert 4 consecutive losing sell trades
        // (negative P&L: sell at price lower than buy, but the query just checks
        //  the computed pnl column)
        for i in 0..4 {
            sqlx::query(
                "INSERT INTO trades (symbol, action, shares, price) VALUES (?, 'sell', 10.0, ?)",
            )
            .bind(format!("LOSS{}", i))
            .bind(-50.0) // negative price = loss in the query's calculation
            .execute(rm.pool())
            .await
            .unwrap();
        }

        let count = rm.get_consecutive_losses().await.unwrap();
        assert!(
            count >= 4,
            "Expected >= 4 consecutive losses, got {}",
            count
        );
    }

    #[tokio::test]
    async fn drawdown_from_peak() {
        let rm = setup_test_db().await;

        // Record a peak
        let dd = rm.check_drawdown_from_peak(100_000.0).await.unwrap();
        assert_eq!(dd, 0.0); // First value = new peak, no drawdown

        // Now drop to 85k = 15% drawdown
        let dd = rm.check_drawdown_from_peak(85_000.0).await.unwrap();
        assert!(
            (dd - 15.0).abs() < 0.1,
            "Expected ~15% drawdown, got {}",
            dd
        );
    }
}
