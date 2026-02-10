# InvestIQ Production Readiness Plan

## Phase 1: Critical Security & Data Integrity (Week 1)

### 1.1 Financial Precision — Replace f64 with Decimal
- Add `rust_decimal` crate to workspace
- Create a `Money` type alias or wrapper in `analysis-core`
- Update all price/shares/P&L fields in: `alpaca-broker` models, `portfolio-manager`, `risk-manager`, `trade_executor.rs`
- Alpaca returns strings — parse to Decimal instead of f64
- Update frontend Python to use `Decimal` where needed (trade execution, portfolio values)

### 1.2 Trade Idempotency
- Add `idempotency_keys` table: `(key TEXT PRIMARY KEY, result TEXT, created_at TEXT)`
- Generate UUID idempotency key per trade in `trade_executor.rs` and `broker_routes.rs`
- Check key before executing, return cached result if duplicate
- Auto-expire keys after 24h via cleanup task

### 1.3 Fix SQL Injection Patterns
- `portfolio-manager/src/trades.rs:50` — LIMIT via format!() → parameterized
- `portfolio-manager/src/alerts.rs:91,146` — datetime via format!() → parameterized
- Audit all `format!()` calls touching SQL across codebase

### 1.4 Require API Keys on Startup
- `api-server/src/main.rs` — check `API_KEYS` env var is non-empty at startup, exit if missing
- Add `REQUIRE_AUTH` env var (default true) so dev mode can skip
- Log warning if auth disabled

### 1.5 Audit and Fix unwrap() in Hot Paths
- Priority files: `alpaca-broker/src/client.rs`, `api-server/src/main.rs`, `broker_routes.rs`
- Replace with `?`, `.unwrap_or()`, or `.context()` (anyhow)
- Leave `.unwrap()` only where guaranteed safe (const/static init, already-validated)

---

## Phase 2: Database & Deployment (Week 2)

### 2.1 Implement sqlx Migrations
- Create `migrations/` directory in workspace root
- Convert `schema.sql` into numbered migration: `001_initial_schema.sql`
- Add migration for pending_trades extra columns: `002_pending_trades_price_order_id.sql`
- Add migration for indexes: `003_add_indexes.sql`
- Add migration for idempotency_keys: `004_idempotency_keys.sql`
- Add migration for audit_log: `005_audit_log.sql`
- Update `portfolio-manager/src/db.rs` to run migrations on startup
- Remove inline `CREATE TABLE IF NOT EXISTS` from route files

### 2.2 Add Database Indexes
```sql
CREATE INDEX IF NOT EXISTS idx_trades_date ON trades(trade_date DESC);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
CREATE INDEX IF NOT EXISTS idx_alerts_status ON alerts(status);
CREATE INDEX IF NOT EXISTS idx_pending_trades_status ON pending_trades(status);
CREATE INDEX IF NOT EXISTS idx_active_risk_positions_status ON active_risk_positions(status);
CREATE INDEX IF NOT EXISTS idx_sentiment_history_symbol ON sentiment_history(symbol);
```

### 2.3 Database Backup Strategy
- Add `scripts/backup_db.sh` — copies portfolio.db with timestamp
- Add cron example: `0 */6 * * * /app/scripts/backup_db.sh`
- Document restore procedure
- Add backup volume mount in docker-compose.yml

### 2.4 Docker Compose Fixes
- Add volume mount for database: `./data:/app/data`
- Add ML service (port 8004) to docker-compose.yml
- Add trading-agent as optional service
- Set `DATABASE_URL=sqlite:/app/data/portfolio.db` in compose env

### 2.5 Audit Log Table
```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    action TEXT NOT NULL,        -- 'trade_executed', 'trade_rejected', 'config_changed', etc.
    resource TEXT NOT NULL,      -- 'trade', 'position', 'alert', 'config'
    resource_id TEXT,
    details TEXT,                -- JSON blob with before/after
    source TEXT NOT NULL         -- 'api', 'agent', 'system'
);
```
- Add `log_audit()` helper function
- Call from: trade execution, position close, alert management, config changes

### 2.6 Startup Env Validation
- Create `config_validator.rs` in api-server
- Check all required env vars exist and are valid format
- Print clear table of missing/invalid vars and exit
- Required: `POLYGON_API_KEY`, `ALPACA_API_KEY`, `ALPACA_SECRET_KEY`, `API_KEYS`
- Warn if optional but recommended are missing: `DISCORD_WEBHOOK_URL`, `DATABASE_URL`

---

## Phase 3: Hardening & Observability (Week 3)

### 3.1 API Rate Limiting
- Re-enable `tower_governor` in api-server
- Configure: 60 req/min per IP for read endpoints, 10 req/min for write endpoints
- Return 429 with Retry-After header

### 3.2 Remove API Key Query Param Support
- Delete query param extraction from `auth.rs`
- Header-only: `X-API-Key` or `Authorization: Bearer`

### 3.3 JSON Structured Logging
- Default to JSON format when `ENVIRONMENT=production`
- Keep human-readable for dev
- Add request_id to all log entries

### 3.4 Log Rotation
- Configure `tracing-appender` with rolling file (daily rotation, 7-day retention)
- Or: log to stdout only, let Docker/systemd handle rotation
- Document approach in deployment guide

### 3.5 DB Transaction Wrapping
- Wrap trade execution + trade logging in `sqlx::Transaction`
- Wrap position close + risk manager close in transaction
- Add transaction to backtest snapshot recording

### 3.6 TLS Documentation
- Document nginx reverse proxy setup with Let's Encrypt
- Add `nginx.conf` example to repo
- Or: add `axum-server` with `rustls` for built-in TLS

### 3.7 Circuit Breaker Persistence
- Fix TODO in `risk_routes.rs` — persist halt state to DB
- On startup, check if trading was halted and maintain state

---

## Phase 4: Post-Launch Polish (Week 4+)

- ML prediction caching (symbol+timestamp key, 5-min TTL)
- Prometheus metrics endpoint (`/metrics` in exposition format)
- Role-based access control (viewer/trader/admin)
- Database encryption at rest (SQLCipher)
- Secrets rotation procedure documentation
- Feature flags (DB-backed toggle table)
- Data retention policy (archive >1yr trades)
- Soft delete for positions/alerts (deleted_at column)
