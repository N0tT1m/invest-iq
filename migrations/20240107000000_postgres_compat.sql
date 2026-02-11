-- PostgreSQL compatibility: ensure CURRENT_TIMESTAMP is used for new columns.
-- Note: existing datetime('now') defaults work on SQLite but not Postgres.
-- For fresh Postgres deployments, use migrations_postgres/ instead.
-- This migration is a no-op on SQLite (existing defaults remain).

-- No-op: SQLite ignores ALTER COLUMN and this migration serves as documentation
-- that new tables/columns should use CURRENT_TIMESTAMP instead of datetime('now').
SELECT 1;
