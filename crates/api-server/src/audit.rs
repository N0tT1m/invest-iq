use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use tokio::sync::Mutex;

/// Serializes audit writes to prevent race conditions on the hash chain.
static AUDIT_WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Log an audit event to the audit_log table with tamper-evident hash chain.
/// Each entry stores a SHA-256 hash of its contents plus the previous entry's hash,
/// forming an append-only verifiable chain.
///
/// Uses a mutex + transaction to ensure the read-prev-hash + insert is atomic,
/// preventing concurrent writes from breaking the hash chain.
pub async fn log_audit(
    pool: &sqlx::AnyPool,
    event_type: &str,
    symbol: Option<&str>,
    action: Option<&str>,
    details: Option<&str>,
    user_id: &str,
    order_id: Option<&str>,
) {
    // Serialize audit writes to prevent race conditions on the hash chain
    let _guard = AUDIT_WRITE_LOCK.lock().await;

    let tx_result: Result<(), sqlx::Error> = async {
        let mut tx = pool.begin().await?;

        // Fetch the previous entry's hash and sequence number within the transaction
        let (prev_hash, prev_seq): (String, i64) = sqlx::query_as(
            "SELECT COALESCE(entry_hash, ''), COALESCE(sequence_number, 0)
             FROM audit_log ORDER BY sequence_number DESC LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or_default();

        let sequence_number = prev_seq + 1;
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let entry_hash = compute_entry_hash(
            &prev_hash,
            event_type,
            symbol.unwrap_or(""),
            action.unwrap_or(""),
            details.unwrap_or(""),
            &timestamp,
        );

        sqlx::query(
            "INSERT INTO audit_log (event_type, symbol, action, details, user_id, order_id, prev_hash, entry_hash, sequence_number, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event_type)
        .bind(symbol)
        .bind(action)
        .bind(details)
        .bind(user_id)
        .bind(order_id)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .bind(sequence_number)
        .bind(&timestamp)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
    .await;

    if let Err(e) = tx_result {
        tracing::warn!("Failed to write audit log ({}): {}", event_type, e);
    }
}

/// Compute a SHA-256 hash for an audit entry.
fn compute_entry_hash(
    prev_hash: &str,
    event_type: &str,
    symbol: &str,
    action: &str,
    details: &str,
    timestamp: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(event_type.as_bytes());
    hasher.update(b"|");
    hasher.update(symbol.as_bytes());
    hasher.update(b"|");
    hasher.update(action.as_bytes());
    hasher.update(b"|");
    hasher.update(details.as_bytes());
    hasher.update(b"|");
    hasher.update(timestamp.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify the integrity of the audit chain.
/// Walks all entries in sequence order and recomputes hashes.
/// Returns (is_valid, total_entries, first_broken_sequence).
pub async fn verify_audit_chain(
    pool: &sqlx::AnyPool,
) -> Result<AuditChainVerification, sqlx::Error> {
    let entries: Vec<AuditChainEntry> = sqlx::query_as(
        "SELECT sequence_number, event_type, symbol, action, details, created_at, prev_hash, entry_hash
         FROM audit_log
         WHERE sequence_number > 0
         ORDER BY sequence_number ASC",
    )
    .fetch_all(pool)
    .await?;

    if entries.is_empty() {
        return Ok(AuditChainVerification {
            is_valid: true,
            total_entries: 0,
            first_broken_sequence: None,
            message: "No hash-chained entries found".to_string(),
        });
    }

    let mut expected_prev_hash = String::new();

    for entry in &entries {
        // Verify prev_hash matches what we expect
        if entry.prev_hash != expected_prev_hash {
            return Ok(AuditChainVerification {
                is_valid: false,
                total_entries: entries.len() as i64,
                first_broken_sequence: Some(entry.sequence_number),
                message: format!(
                    "Chain broken at sequence {}: expected prev_hash '{}', got '{}'",
                    entry.sequence_number,
                    &expected_prev_hash[..expected_prev_hash.len().min(16)],
                    &entry.prev_hash[..entry.prev_hash.len().min(16)],
                ),
            });
        }

        // Recompute hash and verify
        let recomputed = compute_entry_hash(
            &entry.prev_hash,
            &entry.event_type,
            &entry.symbol,
            &entry.action,
            &entry.details,
            &entry.created_at,
        );

        if recomputed != entry.entry_hash {
            return Ok(AuditChainVerification {
                is_valid: false,
                total_entries: entries.len() as i64,
                first_broken_sequence: Some(entry.sequence_number),
                message: format!(
                    "Hash mismatch at sequence {}: entry may have been tampered with",
                    entry.sequence_number,
                ),
            });
        }

        expected_prev_hash = entry.entry_hash.clone();
    }

    Ok(AuditChainVerification {
        is_valid: true,
        total_entries: entries.len() as i64,
        first_broken_sequence: None,
        message: format!("All {} entries verified successfully", entries.len()),
    })
}

#[derive(serde::Serialize)]
pub struct AuditChainVerification {
    pub is_valid: bool,
    pub total_entries: i64,
    pub first_broken_sequence: Option<i64>,
    pub message: String,
}

#[derive(sqlx::FromRow)]
struct AuditChainEntry {
    sequence_number: i64,
    event_type: String,
    #[sqlx(default)]
    symbol: String,
    #[sqlx(default)]
    action: String,
    #[sqlx(default)]
    details: String,
    #[sqlx(default)]
    created_at: String,
    prev_hash: String,
    entry_hash: String,
}
