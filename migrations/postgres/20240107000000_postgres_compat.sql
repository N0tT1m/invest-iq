-- PostgreSQL compatibility: this migration is a no-op.
-- The SQLite version documented that new tables/columns should use CURRENT_TIMESTAMP
-- instead of datetime('now'). In the PostgreSQL migration set, all tables already
-- use TIMESTAMPTZ DEFAULT NOW() natively.

SELECT 1;
