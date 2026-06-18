//! Server-Side Request Forgery (SSRF) protection for outbound URL fetches.
//!
//! Any code path that fetches a *user-supplied* URL server-side (image
//! downloads, NFT `tokenURI` metadata, etc.) must run the URL through
//! [`validate_url_not_internal`] before handing it to an HTTP client.
//!
//! The check enforces two things:
//!  1. A scheme allowlist — only `http`/`https` are permitted. This blocks
//!     local-file reads (`file://`), `gopher://`, `ftp://`, and other schemes
//!     that can be abused for SSRF/LFI.
//!  2. An internal-destination denylist — localhost, private/loopback/link-local
//!     IP ranges, cloud metadata endpoints, and common internal hostnames are
//!     rejected so an attacker cannot pivot to internal services
//!     (e.g. `http://169.254.169.254/...` for cloud credentials).

use crate::error::LibError;
use reqwest::Url;
use std::net::IpAddr;

/// Validates that a URL is safe to fetch server-side (SSRF protection).
///
/// Returns `Err(LibError::SsrfBlocked)` if the scheme is not `http`/`https`,
/// or if the host resolves to an internal/private destination. Returns
/// `Err(LibError::InvalidInput)` if the string is not a parseable URL.
pub fn validate_url_not_internal(url_str: &str) -> Result<(), LibError> {
    let url = Url::parse(url_str).map_err(|_| LibError::InvalidInput(url_str.to_string()))?;

    // 1. Scheme allowlist. Blocks file://, gopher://, ftp://, data://, etc.
    if !matches!(url.scheme(), "http" | "https") {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
    }

    let host = url
        .host_str()
        .ok_or_else(|| LibError::SsrfBlocked(url_str.to_string()))?;

    // 2a. Explicit localhost variants
    if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
    }

    // 2b. Literal IP hosts in a private/internal range.
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(LibError::SsrfBlocked(url_str.to_string()));
        }
    }

    // 2c. Common internal hostnames and the cloud metadata endpoint.
    let host_lower = host.to_lowercase();
    if host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
        || host_lower.ends_with(".localhost")
        || host_lower == "metadata.google.internal"
        || host_lower == "169.254.169.254"
    {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
    }

    Ok(())
}

/// Returns `true` if the IP address belongs to a private, loopback, link-local,
/// or otherwise non-publicly-routable range that should be blocked for SSRF.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()
                || ipv4.is_loopback()
                || ipv4.is_link_local()
                || (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64) // 100.64.0.0/10
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}
