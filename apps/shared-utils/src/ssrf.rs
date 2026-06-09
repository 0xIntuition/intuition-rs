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
///
/// Note: this is a host/scheme-level check and does not perform DNS resolution,
/// so a public hostname that resolves to a private IP (DNS rebinding) is not
/// caught here. The HTTP clients in this codebase are the appropriate layer to
/// add resolved-IP pinning if that threat becomes relevant.
pub fn validate_url_not_internal(url_str: &str) -> Result<(), LibError> {
    let url = Url::parse(url_str).map_err(|_| LibError::InvalidInput(url_str.to_string()))?;

    // 1. Scheme allowlist. Blocks file://, gopher://, ftp://, data://, etc.
    if !matches!(url.scheme(), "http" | "https") {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
    }

    // A http(s) URL without a host is malformed for our purposes.
    let host = url
        .host_str()
        .ok_or_else(|| LibError::SsrfBlocked(url_str.to_string()))?;

    // 2a. Explicit localhost variants (covers the non-IP "localhost" label and
    // bracketed IPv6 forms that don't parse cleanly as a bare IpAddr).
    if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
    }

    // 2b. Literal IP hosts in a private/internal range.
    if let Ok(ip) = host.parse::<IpAddr>()
        && is_private_ip(&ip)
    {
        return Err(LibError::SsrfBlocked(url_str.to_string()));
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
            // Private ranges: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            // Loopback: 127.0.0.0/8
            // Link-local: 169.254.0.0/16
            // Carrier-grade NAT: 100.64.0.0/10
            ipv4.is_private()
                || ipv4.is_loopback()
                || ipv4.is_link_local()
                || (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64) // 100.64.0.0/10
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified() // 0.0.0.0
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                // Unique local addresses (fc00::/7)
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local (fe80::/10)
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn is_ssrf_blocked(url: &str) -> bool {
        matches!(
            validate_url_not_internal(url),
            Err(LibError::SsrfBlocked(_))
        )
    }

    #[test]
    fn blocks_non_http_schemes() {
        // The original report's payload: local file read via file://.
        assert!(is_ssrf_blocked("file:///etc/passwd"));
        assert!(is_ssrf_blocked("file://localhost/etc/passwd"));
        assert!(is_ssrf_blocked("gopher://127.0.0.1:6379/_INFO"));
        assert!(is_ssrf_blocked("ftp://example.com/secret"));
        assert!(is_ssrf_blocked("data:text/plain;base64,SGVsbG8="));
    }

    #[test]
    fn blocks_localhost() {
        assert!(is_ssrf_blocked("http://localhost/x"));
        assert!(is_ssrf_blocked("http://127.0.0.1/x"));
        assert!(is_ssrf_blocked("http://127.1.2.3/x")); // anywhere in 127.0.0.0/8
        assert!(is_ssrf_blocked("http://[::1]/x"));
    }

    #[test]
    fn blocks_private_and_link_local_ips() {
        // 10.0.0.0/8
        assert!(is_ssrf_blocked("http://10.0.0.1/x"));
        assert!(is_ssrf_blocked("http://10.255.255.255/x"));
        // 172.16.0.0/12
        assert!(is_ssrf_blocked("http://172.16.0.1/x"));
        assert!(is_ssrf_blocked("http://172.31.255.255/x"));
        // 192.168.0.0/16
        assert!(is_ssrf_blocked("http://192.168.0.1/x"));
        // Link-local + cloud metadata
        assert!(is_ssrf_blocked("http://169.254.169.254/latest/meta-data/"));
        // Unspecified
        assert!(is_ssrf_blocked("http://0.0.0.0/x"));
        // Carrier-grade NAT 100.64.0.0/10
        assert!(is_ssrf_blocked("http://100.64.0.1/x"));
    }

    #[test]
    fn blocks_internal_hostnames() {
        assert!(is_ssrf_blocked("http://myservice.local/x"));
        assert!(is_ssrf_blocked("http://internal.internal/x"));
        assert!(is_ssrf_blocked("http://test.localhost/x"));
        assert!(is_ssrf_blocked("http://metadata.google.internal/x"));
    }

    #[test]
    fn rejects_unparseable_input() {
        assert!(matches!(
            validate_url_not_internal("not a url"),
            Err(LibError::InvalidInput(_))
        ));
    }

    #[test]
    fn allows_public_urls() {
        assert!(validate_url_not_internal("https://example.com/image.png").is_ok());
        assert!(validate_url_not_internal("https://ipfs.io/ipfs/QmTest").is_ok());
        assert!(validate_url_not_internal("https://arweave.net/test").is_ok());
        assert!(validate_url_not_internal("http://8.8.8.8/test").is_ok());
        // 100.x outside the 100.64.0.0/10 CGNAT block is public.
        assert!(validate_url_not_internal("http://100.0.0.1/test").is_ok());
    }

    #[test]
    fn is_private_ip_classifies_correctly() {
        // Private / non-routable IPv4
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        // Public IPv4
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        // Non-routable / private IPv6
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(
            0xfc00, 0, 0, 0, 0, 0, 0, 1
        ))));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        ))));
        // Public IPv6
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 1
        ))));
    }
}
