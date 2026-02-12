use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use ipnet::IpNet;
use std::net::SocketAddr;

/// Parsed list of allowed IP networks for admin endpoints.
#[derive(Clone)]
pub struct IpAllowlist {
    networks: Vec<IpNet>,
}

impl IpAllowlist {
    /// Parse `ADMIN_IP_ALLOWLIST` env var (comma-separated CIDRs).
    /// Returns `None` if the var is unset or empty (dev mode: allow all).
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var("ADMIN_IP_ALLOWLIST").ok()?;
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        let networks: Vec<IpNet> = raw
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                match s.parse::<IpNet>() {
                    Ok(net) => Some(net),
                    Err(e) => {
                        tracing::warn!("Invalid CIDR in ADMIN_IP_ALLOWLIST: '{}': {}", s, e);
                        None
                    }
                }
            })
            .collect();

        if networks.is_empty() {
            tracing::warn!("ADMIN_IP_ALLOWLIST set but no valid CIDRs parsed");
            return None;
        }

        tracing::info!(
            "Admin IP allowlist: {} network(s): {:?}",
            networks.len(),
            networks
        );
        Some(Self { networks })
    }

    pub fn is_allowed(&self, addr: &std::net::IpAddr) -> bool {
        self.networks.iter().any(|net| net.contains(addr))
    }
}

/// Middleware that restricts access to allowed IP ranges.
///
/// Pass `Option<IpAllowlist>` as state:
/// - `None` = dev mode, all IPs allowed
/// - `Some(allowlist)` = only listed CIDRs may access
pub async fn ip_allowlist_middleware(
    axum::extract::State(allowlist): axum::extract::State<Option<IpAllowlist>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref al) = allowlist {
        match connect_info.map(|ci| ci.0.ip()) {
            Some(ip) if al.is_allowed(&ip) => {}
            Some(ip) => {
                tracing::warn!(
                    "Admin endpoint access denied for IP {} (not in allowlist)",
                    ip
                );
                return Err(StatusCode::FORBIDDEN);
            }
            None => {
                tracing::warn!("Admin endpoint access denied: client IP unavailable");
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    Ok(next.run(request).await)
}
