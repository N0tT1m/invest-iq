-- Tamper-evident audit chain: add hash chain columns to audit_log
ALTER TABLE audit_log ADD COLUMN prev_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE audit_log ADD COLUMN entry_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE audit_log ADD COLUMN sequence_number INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_audit_log_sequence ON audit_log(sequence_number);
