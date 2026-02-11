#!/bin/bash
# Database backup script for InvestIQ
# Usage: ./scripts/backup-db.sh [backup_dir]
# Recommended: run via cron, e.g. 0 2 * * * /path/to/backup-db.sh

set -euo pipefail

DB_PATH="${DATABASE_URL:-portfolio.db}"
DB_PATH="${DB_PATH#sqlite:}"  # Strip sqlite: prefix if present

BACKUP_DIR="${1:-./backups}"
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-7}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="${BACKUP_DIR}/portfolio_${TIMESTAMP}.db"

# Create backup directory
mkdir -p "${BACKUP_DIR}"

if [ ! -f "${DB_PATH}" ]; then
    echo "Database not found at ${DB_PATH}"
    exit 1
fi

# Use SQLite's backup command for a consistent copy
if command -v sqlite3 &> /dev/null; then
    sqlite3 "${DB_PATH}" ".backup '${BACKUP_FILE}'"
else
    # Fallback to file copy (safe when WAL mode is enabled)
    cp "${DB_PATH}" "${BACKUP_FILE}"
    [ -f "${DB_PATH}-wal" ] && cp "${DB_PATH}-wal" "${BACKUP_FILE}-wal"
    [ -f "${DB_PATH}-shm" ] && cp "${DB_PATH}-shm" "${BACKUP_FILE}-shm"
fi

echo "Backup created: ${BACKUP_FILE} ($(du -h "${BACKUP_FILE}" | cut -f1))"

# Clean up old backups
DELETED=$(find "${BACKUP_DIR}" -name "portfolio_*.db*" -mtime +${RETENTION_DAYS} -delete -print | wc -l)
if [ "${DELETED}" -gt 0 ]; then
    echo "Cleaned up ${DELETED} backup file(s) older than ${RETENTION_DAYS} days"
fi
