# InvestIQ Production Readiness Status

All items from the original production plan have been completed. This document tracks what was done and any remaining polish work.

## Completed (Phases 1-3)

### Phase 1: Security & Data Integrity
- [x] **Financial precision** — `rust_decimal` with `serde-with-float` for JSON, `#[sqlx(try_from = "f64")]` for DB reads
- [x] **Trade idempotency** — `trade_idempotency` table, 24h expiry, checked before order submission
- [x] **SQL injection fixes** — Parameterized LIMIT/status queries in `trades.rs`, `alerts.rs`
- [x] **Auth enforcement** — `REQUIRE_AUTH=true` exits if `API_KEYS` empty; header-only auth (no query params)
- [x] **unwrap() cleanup** — `.expect()` with messages in `alpaca-broker/client.rs`; `?` in hot paths

### Phase 2: Database & Deployment
- [x] **sqlx migrations** — `migrations/` directory, 2 migration files, all 26+ tables consolidated
- [x] **Database indexes** — Included in initial migration
- [x] **Backup strategy** — `scripts/backup-db.sh` + Docker `db-backup` sidecar
- [x] **Docker Compose** — `db_data` volume, `signal-models` service, `trading-agent` profile, log rotation
- [x] **Audit logging** — `audit.rs` module, logged from broker/agent/risk routes
- [x] **Startup validation** — Safety gate for live trading (`LIVE_TRADING_APPROVED`), `REQUIRE_AUTH` check

### Phase 3: Hardening & Observability
- [x] **Rate limiting** — `tower_governor` with `SmartIpKeyExtractor`, `RATE_LIMIT_PER_MINUTE` env var
- [x] **JSON structured logging** — `RUST_LOG_FORMAT=json` for production
- [x] **DB transactions** — Trade execution wraps log + risk position + portfolio update in sqlx transaction
- [x] **TLS documentation** — `docs/deployment.md` with nginx/Caddy/Traefik examples
- [x] **Circuit breaker persistence** — `trade_outcomes` table, `record_trade_outcome()`, real consecutive loss tracking
- [x] **Risk target profiles** — `get_target_profile` / `update_target_profile` persist to `risk_target_profile` table

### Stub Fixes (all implemented)
- [x] **scan_top_movers** — Polygon snapshot-based scanning of 20 liquid large-caps
- [x] **retire_strategy** — Updates `strategy_health_snapshots` status to 'retired'
- [x] **position_pnl** — Calculates from entry_price and current_price
- [x] **get_watchlist_items** — Queries `watchlist` table
- [x] **Dead file cleanup** — Removed unused `ml_strategy_manager.rs`

## Remaining Polish (Phase 4 — Optional)

These are nice-to-haves, not blockers for production paper trading:

- [ ] ML prediction caching (symbol+timestamp key, 5-min TTL)
- [ ] Prometheus metrics endpoint (`/metrics`)
- [ ] Role-based access control (viewer/trader/admin)
- [ ] Database encryption at rest (SQLCipher)
- [ ] Secrets rotation documentation
- [ ] Data retention policy (archive >1yr trades)
- [ ] Cleanup compiler warnings across workspace
