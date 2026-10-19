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
    let url = Url::parse(url_str).map_err(|_| LibError::InvalidInput("unparseable url".into()))?;
    validate_parsed_url(&url)
}

/// Same policy as [`validate_url_not_internal`] for an already parsed URL, so
/// callers that hold a [`Url`] do not parse twice.
///
/// Error payloads carry a redacted form of the URL (see [`redact_url`]), so they
/// are safe to log.
pub fn validate_parsed_url(url: &Url) -> Result<(), LibError> {
    let blocked = || LibError::SsrfBlocked(redact_url(url.as_str()));

    // 1. Scheme allowlist. Blocks file://, gopher://, ftp://, data://, etc.
    if !matches!(url.scheme(), "http" | "https") {
        return Err(blocked());
    }

    // 2. Host checks. `Url::host()` is used (not `host_str()`) because
    // `host_str()` keeps the brackets of an IPv6 literal (`[fe80::1]`).
    match url.host() {
        // A http(s) URL without a host is malformed for our purposes.
        None => Err(blocked()),
        Some(Host::Domain(domain)) if is_internal_hostname(domain) => Err(blocked()),
        Some(Host::Domain(_)) => Ok(()),
        Some(Host::Ipv4(v4)) if is_private_ip(&IpAddr::V4(v4)) => Err(blocked()),
        Some(Host::Ipv6(v6)) if is_private_ip(&IpAddr::V6(v6)) => Err(blocked()),
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => Ok(()),
    }
}

/// The IP address of a literal-IP URL host, if the host is one.
pub(crate) fn literal_ip(url: &Url) -> Option<IpAddr> {
    match url.host() {
        Some(Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    }
}

/// Host-name denylist: `localhost`, the `.localhost`, `.local` and `.internal`
/// suffixes (which also cover the cloud metadata name). The name is normalised
/// first: a trailing root dot (`localhost.`) names the same host and must not
/// slip past the string checks, and comparison is ASCII case-insensitive.
fn is_internal_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
}

/// Reduces a URL to `scheme://host[:port]/path` for logging and error payloads:
/// userinfo, query and fragment are dropped. Non-http(s) URLs (for example huge
/// `data:` URLs) and unparseable input are replaced by a fixed placeholder.
pub fn redact_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {
            let host = url.host_str().unwrap_or("");
            let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
            format!("{}://{host}{port}{}", url.scheme(), url.path())
        }
        Ok(url) => format!("{}:<redacted>", url.scheme()),
        Err(_) => "<unparseable url>".to_string(),
    }
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
        || (s[0] == 0x2001 && (s[1] & 0xfe00) == 0) // IETF protocol assignments 2001::/23 (incl. Teredo 2001::/32)
        || s[0] == 0x5f00 // SRv6 SIDs 5f00::/16
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation 2001:db8::/32
        || (s[0] == 0x3fff && (s[1] & 0xf000) == 0) // documentation 3fff::/20
}

mod fetch;

pub use fetch::{
    FETCH_CONNECT_TIMEOUT, FETCH_MAX_REDIRECTS, FETCH_TOTAL_TIMEOUT, FetchLimits, FetchOutcome,
    HostResolver, SystemResolver, fetch_public_bytes, validate_fetch_url,
};

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
        // Non-http(s) schemes such as file:// must never be fetched.
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
    fn blocks_hostnames_with_trailing_dot_and_mixed_case() {
        // A trailing root dot names the same host; the denylist must see through it.
        for u in [
            "http://localhost./x",
            "http://localhost../x",
            "http://x.internal./x",
            "http://metadata.google.internal./x",
            "http://foo.localhost./x",
            "http://svc.local./x",
            "http://LOCALHOST/x",
            "http://Metadata.Google.Internal./x",
        ] {
            assert!(is_ssrf_blocked(u), "{u} must be blocked");
        }
        // Names that merely contain the labels stay allowed.
        for u in [
            "https://localhost.example.com/x",
            "https://internal.example.com/x",
            "https://notlocal/x",
            "https://example.com./x",
        ] {
            assert!(validate_url_not_internal(u).is_ok(), "{u} must be allowed");
        }
    }

    #[test]
    fn validate_parsed_url_matches_string_validator() {
        for u in [
            "http://localhost./x",
            "https://example.com/x",
            "ftp://example.com/x",
            "http://[fe80::1]/x",
        ] {
            let parsed = Url::parse(u).unwrap();
            assert_eq!(
                validate_parsed_url(&parsed).is_ok(),
                validate_url_not_internal(u).is_ok(),
                "{u}"
            );
        }
    }

    #[test]
    fn error_payloads_carry_redacted_urls_only() {
        let Err(LibError::SsrfBlocked(payload)) =
            validate_url_not_internal("http://user:pw@127.0.0.1:8080/a/b?token=SECRET#frag")
        else {
            panic!("expected SsrfBlocked");
        };
        assert_eq!(payload, "http://127.0.0.1:8080/a/b");
        assert_eq!(
            redact_url("https://u:p@example.com/p?q=1"),
            "https://example.com/p"
        );
        assert_eq!(redact_url("data:image/png;base64,AAAA"), "data:<redacted>");
        assert_eq!(redact_url("not a url"), "<unparseable url>");
        let Err(LibError::InvalidInput(msg)) = validate_url_not_internal("not a url SECRET") else {
            panic!("expected InvalidInput");
        };
        assert!(!msg.contains("SECRET"));
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
            "2001:1::1",   // PCP anycast, inside 2001::/23
            "2001:2::1",   // benchmarking, inside 2001::/23
            "2001:1ff::1", // last /32 block of 2001::/23
            "5f00::1",     // SRv6 SIDs 5f00::/16
            "5f00:ffff::1",
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
            "2001:200::1", // first address after 2001::/23
            "2001:4860::1",
            "5eff:ffff::1", // just below 5f00::/16
            "5f01::1",      // just above 5f00::/16
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
