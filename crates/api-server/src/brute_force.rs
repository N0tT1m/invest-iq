use dashmap::DashMap;
use std::time::Instant;

struct FailureRecord {
    count: u32,
    first_failure: Instant,
    locked_until: Option<Instant>,
}

/// IP-based authentication failure tracker with automatic lockout.
///
/// Tracks failed auth attempts per IP. After `max_failures` within `window`,
/// the IP is locked out for `lockout` duration.
pub struct BruteForceGuard {
    failures: DashMap<String, FailureRecord>,
    max_failures: u32,
    window: std::time::Duration,
    lockout: std::time::Duration,
}

impl BruteForceGuard {
    pub fn new() -> Self {
        let max_failures = std::env::var("AUTH_MAX_FAILURES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5u32);
        let window_secs = std::env::var("AUTH_FAILURE_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300u64);
        let lockout_secs = std::env::var("AUTH_LOCKOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900u64);

        tracing::info!(
            "Brute-force guard: max {} failures in {}s window, {}s lockout",
            max_failures,
            window_secs,
            lockout_secs
        );

        Self {
            failures: DashMap::new(),
            max_failures,
            window: std::time::Duration::from_secs(window_secs),
            lockout: std::time::Duration::from_secs(lockout_secs),
        }
    }

    /// Record an authentication failure for the given IP.
    /// Triggers lockout after `max_failures` within the tracking window.
    pub fn record_failure(&self, ip: &str) {
        let now = Instant::now();
        let mut entry = self
            .failures
            .entry(ip.to_string())
            .or_insert(FailureRecord {
                count: 0,
                first_failure: now,
                locked_until: None,
            });
        let record = entry.value_mut();

        // Reset if outside tracking window
        if now.duration_since(record.first_failure) > self.window {
            record.count = 0;
            record.first_failure = now;
            record.locked_until = None;
        }

        record.count += 1;
        if record.count >= self.max_failures {
            record.locked_until = Some(now + self.lockout);
            tracing::warn!(
                "Brute-force lockout triggered for IP {} ({} failures in window)",
                ip,
                record.count
            );
        }
    }

    /// Check if an IP is currently locked out.
    pub fn is_locked(&self, ip: &str) -> bool {
        if let Some(entry) = self.failures.get(ip) {
            if let Some(locked_until) = entry.locked_until {
                if Instant::now() < locked_until {
                    return true;
                }
            }
        }
        false
    }

    /// Clear failure tracking for an IP after successful authentication.
    pub fn record_success(&self, ip: &str) {
        self.failures.remove(ip);
    }

    /// Remove stale entries older than window + lockout duration.
    /// Called periodically by a background task.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let max_age = self.window + self.lockout;
        self.failures
            .retain(|_, record| now.duration_since(record.first_failure) < max_age);
    }
}
