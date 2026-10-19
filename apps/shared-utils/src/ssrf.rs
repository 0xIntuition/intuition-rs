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
use reqwest::{Url, header::LOCATION, redirect};
use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    time::Duration,
};
use url::Host;

/// Validates that a URL is safe to fetch server-side (SSRF protection).
///
/// Returns `Err(LibError::SsrfBlocked)` if the scheme is not `http`/`https`,
/// or if the host resolves to an internal/private destination. Returns
/// `Err(LibError::InvalidInput)` if the string is not a parseable URL.
///
/// Note: this is a host/scheme-level check and does not perform DNS resolution,
/// so a public hostname that resolves to a private IP (DNS rebinding) is not
/// caught here. Code that actually fetches a user-supplied URL must use
/// [`fetch_public_bytes`], which resolves, validates every answer, pins the
/// connection and re-validates each redirect hop.
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

    // 2b. Literal IP hosts in a private/internal range. `Url::host()` is used
    // (not `host_str().parse()`) because `host_str()` keeps the brackets of an
    // IPv6 literal (`[fe80::1]`), which never parses as a bare `IpAddr`.
    let literal_ip = match url.host() {
        Some(Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };
    if let Some(ip) = literal_ip
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
/// reserved, multicast, or otherwise non-publicly-routable range that should be
/// blocked for SSRF.
///
/// IPv6 addresses that embed an IPv4 address (IPv4-mapped `::ffff:a.b.c.d`,
/// IPv4-compatible `::a.b.c.d`, NAT64 `64:ff9b::/96` and 6to4 `2002::/16`) are
/// classified by the embedded IPv4 address, so they cannot be used to smuggle
/// `127.0.0.1` or `169.254.169.254` past the filter.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_private_ipv6(ipv6),
    }
}

fn is_private_ipv4(ip: &std::net::Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private() // 10/8, 172.16/12, 192.168/16
        || ip.is_loopback() // 127/8
        || ip.is_link_local() // 169.254/16 (cloud metadata)
        || ip.is_broadcast()
        || ip.is_documentation() // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || ip.is_multicast() // 224/4
        || o[0] == 0 // "this network" 0.0.0.0/8 (includes unspecified)
        || (o[0] == 100 && (o[1] & 0xC0) == 64) // CGNAT 100.64/10
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF protocol assignments 192.0.0/24
        || (o[0] == 192 && o[1] == 88 && o[2] == 99) // deprecated 6to4 relay anycast
        || (o[0] == 198 && (o[1] & 0xFE) == 18) // benchmarking 198.18/15
        || o[0] >= 240 // reserved 240/4 (includes 255.255.255.255)
}

fn is_private_ipv6(ip: &std::net::Ipv6Addr) -> bool {
    // IPv4-mapped (::ffff:0:0/96) and IPv4-compatible (::/96): judge by the v4 part.
    if let Some(v4) = ip.to_ipv4() {
        return is_private_ipv4(&v4);
    }
    let s = ip.segments();
    let embedded = |hi: u16, lo: u16| {
        let [a, b] = hi.to_be_bytes();
        let [c, d] = lo.to_be_bytes();
        is_private_ipv4(&std::net::Ipv4Addr::new(a, b, c, d))
    };
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast() // ff00::/8
        || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (s[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (s[0] & 0xffc0) == 0xfec0 // deprecated site-local fec0::/10
        || (s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0) // discard-only 100::/64
        || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0
            && embedded(s[6], s[7])) // NAT64 64:ff9b::/96 embedding a private v4
        || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 1) // local-use NAT64 64:ff9b:1::/48
        || (s[0] == 0x2002 && embedded(s[1], s[2])) // 6to4 2002::/16 embedding a private v4
        || (s[0] == 0x2001 && s[1] == 0) // Teredo 2001::/32 (embeds obfuscated v4)
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation 2001:db8::/32
        || (s[0] == 0x3fff && (s[1] & 0xf000) == 0) // documentation 3fff::/20
}

/// Fetch limits for [`fetch_public_bytes`].
///
/// The defaults are the production policy. `allow_http` is an explicit opt-in
/// that no production caller sets (the only caller, `Image::download`, serves
/// user-supplied URLs); it exists so tests can talk to a plain-TCP local server.
#[derive(Debug, Clone)]
pub struct FetchLimits {
    /// Hard cap on the response body. Aligned with the decoded image cap.
    pub max_bytes: usize,
    /// TCP connect timeout per hop.
    pub connect_timeout: Duration,
    /// Deadline for the entire fetch including all redirect hops and DNS.
    pub total_timeout: Duration,
    /// Maximum number of redirect hops that are followed (each re-validated).
    pub max_redirects: usize,
    /// Permit plain `http://` URLs. `false` (https only) by default.
    pub allow_http: bool,
}

/// Connect timeout: public CDNs/gateways answer in well under a second; 5s keeps
/// unroutable-address probes from tying up a worker.
pub const FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Total deadline: generous for a 50 MiB image over a slow link, short enough
/// that a slow-loris origin cannot hold a request open for long.
pub const FETCH_TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
/// Maximum redirect hops followed manually (typical CDN chains are 1-2).
pub const FETCH_MAX_REDIRECTS: usize = 3;

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            max_bytes: crate::image::MAX_IMAGE_SIZE,
            connect_timeout: FETCH_CONNECT_TIMEOUT,
            total_timeout: FETCH_TOTAL_TIMEOUT,
            max_redirects: FETCH_MAX_REDIRECTS,
            allow_http: false,
        }
    }
}

/// Result of a completed fetch.
#[derive(Debug)]
pub enum FetchOutcome {
    /// The origin answered `200 OK`; the full (size-capped) body.
    Body(Vec<u8>),
    /// The origin answered with a non-200, non-followed status.
    UnexpectedStatus(u16),
}

/// Resolves a host name to socket addresses. Abstracted so tests can inject
/// answers (rebinding, private targets) without real DNS.
pub trait HostResolver: Send + Sync {
    /// Resolve `host` for `port`.
    fn resolve<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send + 'a>>;
}

/// The system resolver (`getaddrinfo` via tokio, on the blocking pool).
pub struct SystemResolver;

impl HostResolver for SystemResolver {
    fn resolve<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send + 'a>> {
        Box::pin(async move { Ok(tokio::net::lookup_host((host, port)).await?.collect()) })
    }
}

/// Fetch a user-supplied URL with SSRF protection.
///
/// Policy (see `docs/eng-16792-image-guard-ssrf-hardening.md`):
/// * https only (unless `limits.allow_http`), no URL credentials;
/// * the host is resolved here and rejected if ANY address is private per
///   [`is_private_ip`]; the connection is then pinned to exactly the validated
///   addresses so a DNS rebind between check and connect cannot win. TLS SNI and
///   certificate verification still use the host name;
/// * redirects are followed manually (at most `limits.max_redirects`) because
///   reqwest's automatic redirects would skip validation of every hop. Each hop
///   repeats scheme, host, DNS and IP validation. Manual handling was chosen over
///   "no redirects" because IPFS gateways and CDNs legitimately redirect;
/// * proxies from the environment are ignored (a proxy would resolve names itself
///   and defeat pinning);
/// * the body is streamed and aborted past `limits.max_bytes`.
pub async fn fetch_public_bytes(url: &str, limits: &FetchLimits) -> Result<FetchOutcome, LibError> {
    fetch_with(url, limits, &SystemResolver, is_private_ip).await
}

/// Core of [`fetch_public_bytes`] with injectable resolver and IP policy so the
/// redirect/DNS logic is testable against a loopback server. Production code
/// only reaches it through [`fetch_public_bytes`].
pub(crate) async fn fetch_with(
    url: &str,
    limits: &FetchLimits,
    resolver: &dyn HostResolver,
    is_blocked: fn(&IpAddr) -> bool,
) -> Result<FetchOutcome, LibError> {
    tokio::time::timeout(
        limits.total_timeout,
        fetch_loop(url, limits, resolver, is_blocked),
    )
    .await
    .map_err(|_| LibError::TimeoutError("fetch exceeded total timeout".into()))?
}

async fn fetch_loop(
    url: &str,
    limits: &FetchLimits,
    resolver: &dyn HostResolver,
    is_blocked: fn(&IpAddr) -> bool,
) -> Result<FetchOutcome, LibError> {
    let mut current = Url::parse(url).map_err(|_| LibError::InvalidInput(url.to_string()))?;

    for hop in 0..=limits.max_redirects {
        let pinned = validate_hop(&current, limits, resolver, is_blocked).await?;

        let mut builder = reqwest::Client::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            .no_proxy();
        if let (Some(addrs), Some(host)) = (pinned, current.host_str()) {
            builder = builder.resolve_to_addrs(host, &addrs);
        }
        let client = builder.build()?;

        let mut response = client
            .get(current.clone())
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();

        if status.is_redirection()
            && let Some(location) = response.headers().get(LOCATION)
        {
            if hop == limits.max_redirects {
                return Err(LibError::NetworkError("too many redirects".into()));
            }
            let location = location
                .to_str()
                .map_err(|_| LibError::NetworkError("invalid redirect location".into()))?;
            current = current
                .join(location)
                .map_err(|_| LibError::NetworkError("invalid redirect location".into()))?;
            continue;
        }

        if status != reqwest::StatusCode::OK {
            return Ok(FetchOutcome::UnexpectedStatus(status.as_u16()));
        }

        if let Some(len) = response.content_length()
            && len > limits.max_bytes as u64
        {
            return Err(LibError::PayloadTooLarge(format!(
                "content-length {len} exceeds {}",
                limits.max_bytes
            )));
        }

        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_reqwest)? {
            if body.len().saturating_add(chunk.len()) > limits.max_bytes {
                return Err(LibError::PayloadTooLarge(format!(
                    "body exceeds {} bytes",
                    limits.max_bytes
                )));
            }
            body.extend_from_slice(&chunk);
        }
        return Ok(FetchOutcome::Body(body));
    }

    Err(LibError::NetworkError("too many redirects".into()))
}

/// Classify a transport error: timeouts become [`LibError::TimeoutError`].
fn map_reqwest(err: reqwest::Error) -> LibError {
    if err.is_timeout() {
        LibError::TimeoutError("request timed out".into())
    } else {
        LibError::Reqwest(err)
    }
}

/// Synchronous, network-free part of the fetch policy: scheme (https only unless
/// `limits.allow_http`), no URL credentials, host-name denylist and literal-IP
/// checks. Callers can run it up front to reject obviously bad URLs before doing
/// any other work (e.g. a database lookup); [`fetch_public_bytes`] re-runs it on
/// every hop.
pub fn validate_fetch_url(url: &Url, limits: &FetchLimits) -> Result<(), LibError> {
    let scheme_ok = url.scheme() == "https" || (limits.allow_http && url.scheme() == "http");
    if !scheme_ok || !url.username().is_empty() || url.password().is_some() {
        return Err(LibError::SsrfBlocked(url.to_string()));
    }
    // Host-name denylist (.local, .internal, localhost, metadata) and literal IPs.
    validate_url_not_internal(url.as_str())
}

/// Validate one hop (the initial URL or a redirect target). Returns the
/// addresses to pin the connection to, or `None` for a literal-IP host.
async fn validate_hop(
    url: &Url,
    limits: &FetchLimits,
    resolver: &dyn HostResolver,
    is_blocked: fn(&IpAddr) -> bool,
) -> Result<Option<Vec<SocketAddr>>, LibError> {
    validate_fetch_url(url, limits)?;

    let port = url
        .port_or_known_default()
        .ok_or_else(|| LibError::SsrfBlocked(url.to_string()))?;

    match url.host() {
        Some(Host::Ipv4(v4)) => {
            if is_blocked(&IpAddr::V4(v4)) {
                return Err(LibError::SsrfBlocked(url.to_string()));
            }
            Ok(None)
        }
        Some(Host::Ipv6(v6)) => {
            if is_blocked(&IpAddr::V6(v6)) {
                return Err(LibError::SsrfBlocked(url.to_string()));
            }
            Ok(None)
        }
        Some(Host::Domain(domain)) => {
            let addrs = resolver
                .resolve(domain, port)
                .await
                .map_err(|_| LibError::NetworkError("dns resolution failed".into()))?;
            if addrs.is_empty() {
                return Err(LibError::NetworkError("dns resolution failed".into()));
            }
            // ANY private answer rejects the host: a mixed answer set is the
            // classic rebinding trick.
            if addrs.iter().any(|a| is_blocked(&a.ip())) {
                return Err(LibError::SsrfBlocked(url.to_string()));
            }
            Ok(Some(addrs))
        }
        None => Err(LibError::SsrfBlocked(url.to_string())),
    }
}

#[cfg(test)]
mod fetch_tests;

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

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn blocks_ipv4_mapped_and_compatible_ipv6() {
        for a in [
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.0.0.1",
            "::ffff:192.168.1.1",
            "::127.0.0.1",
            "::ffff:0.0.0.0",
        ] {
            assert!(is_private_ip(&ip(a)), "{a} must be blocked");
        }
        // mapped *public* v4 is still public
        assert!(!is_private_ip(&ip("::ffff:8.8.8.8")));
    }

    #[test]
    fn blocks_nat64_and_6to4_embedding_private_v4() {
        for a in [
            "64:ff9b::7f00:1",    // 127.0.0.1
            "64:ff9b::a9fe:a9fe", // 169.254.169.254
            "64:ff9b::a00:1",     // 10.0.0.1
            "64:ff9b:1::1",       // local-use NAT64
            "2002:7f00:1::1",     // 6to4 of 127.0.0.1
            "2002:a9fe:a9fe::1",  // 6to4 of 169.254.169.254
            "2002:c0a8:101::1",   // 6to4 of 192.168.1.1
        ] {
            assert!(is_private_ip(&ip(a)), "{a} must be blocked");
        }
        // Same wrappers around a PUBLIC v4 (8.8.8.8) stay allowed.
        assert!(!is_private_ip(&ip("64:ff9b::808:808")));
        assert!(!is_private_ip(&ip("2002:808:808::1")));
    }

    #[test]
    fn blocks_reserved_special_use_and_multicast_ranges() {
        for a in [
            "0.0.0.1",
            "0.255.255.255",
            "192.0.0.8",
            "198.18.0.1",
            "198.19.255.254",
            "240.0.0.1",
            "250.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "239.255.255.250",
            "192.88.99.1",
            "100.127.255.255",
            "ff02::1",
            "ff00::",
            "2001:db8::1",
            "2001:db8:ffff::1",
            "2001:0:4136:e378::1",
            "fec0::1",
            "100::1",
            "::",
            "::1",
        ] {
            assert!(is_private_ip(&ip(a)), "{a} must be blocked");
        }
    }

    #[test]
    fn range_edges_do_not_over_block_public_space() {
        for a in [
            "198.17.255.255",
            "198.20.0.0",
            "223.255.255.255",
            "192.0.1.1",
            "192.0.3.1",
            "100.63.255.255",
            "100.128.0.0",
            "1.1.1.1",
            "2606:4700::1111",
            "2a00:1450:4001::200e",
            "2001:4860:4860::8888",
            "2001:1::1",
        ] {
            assert!(!is_private_ip(&ip(a)), "{a} must be allowed");
        }
    }

    #[test]
    fn validate_blocks_bracketed_ipv6_literals_and_mapped_forms() {
        // `host_str()` keeps brackets, so these used to slip through the literal check.
        for u in [
            "http://[fe80::1]/x",
            "http://[fc00::1]/x",
            "http://[::ffff:127.0.0.1]/x",
            "http://[::ffff:169.254.169.254]/x",
            "http://[64:ff9b::a9fe:a9fe]/x",
            "http://[2002:7f00:1::]/x",
            "http://[2001:db8::1]/x",
            "https://[ff02::1]/x",
            "http://198.18.0.1/x",
            "http://240.0.0.1/x",
            "http://224.0.0.1/x",
            "http://0.1.2.3/x",
        ] {
            assert!(is_ssrf_blocked(u), "{u} must be blocked");
        }
        assert!(validate_url_not_internal("https://[2606:4700::1111]/x").is_ok());
    }
}
