//! Guarded outbound fetch of user-supplied URLs.
//!
//! One shared [`reqwest::Client`] is used for every request. It never follows
//! redirects itself, ignores proxy environment variables, and resolves names
//! through [`GuardedDns`], which rejects a host when ANY of its addresses is
//! blocked and hands the connector only validated addresses. Because the
//! connector uses exactly the answer that was validated, there is no window
//! between a validation lookup and the connect-time lookup.
//!
//! Redirects are followed manually, one hop at a time, and each hop repeats the
//! scheme, credential, host-name and literal-IP checks (literal-IP hosts never
//! reach the DNS resolver, so they are checked explicitly).

use super::{is_private_ip, literal_ip, redact_url, validate_parsed_url};
use crate::error::LibError;
use log::debug;
use reqwest::{
    Response, StatusCode, Url,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{HeaderValue, LOCATION},
    redirect,
};
use std::{
    error::Error as StdError,
    fmt,
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::{Arc, LazyLock},
    time::Duration,
};

/// Fetch limits for [`fetch_public_bytes`].
///
/// The defaults are the production policy: https only. Plain `http://` can only
/// be enabled through [`FetchLimits::token_metadata`] (for one documented
/// caller) or, in this crate's tests, `allow_plain_http`; the field itself is
/// crate-private so no other crate can opt in.
#[derive(Debug, Clone)]
pub struct FetchLimits {
    /// Hard cap on the response body. Aligned with the decoded image cap.
    pub max_bytes: usize,
    /// The single deadline for a whole fetch: DNS, connect, every redirect hop
    /// and the body. There is deliberately no second per-request timeout.
    pub total_timeout: Duration,
    /// Maximum number of redirect hops that are followed (each re-validated).
    pub max_redirects: usize,
    /// Permit plain `http://` URLs. `false` (https only) by default.
    pub(crate) allow_http: bool,
}

/// TCP connect timeout per hop (a property of the shared client): public
/// CDNs/gateways answer in well under a second; 5s keeps unroutable-address
/// probes from tying up a worker.
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
            total_timeout: FETCH_TOTAL_TIMEOUT,
            max_redirects: FETCH_MAX_REDIRECTS,
            allow_http: false,
        }
    }
}

impl FetchLimits {
    /// Limits for fetching NFT `tokenURI` metadata documents (the consumer's
    /// CAIP-22 resolver).
    ///
    /// This is the only constructor that permits plain `http://`: ERC-721 token
    /// URIs are chosen by contract deployers and a share of them are plain-http
    /// links to metadata servers, which the resolver has always accepted. Every
    /// other protection (DNS validation, literal-IP checks, per-hop redirect
    /// validation, size and time caps) applies unchanged. Image downloads must
    /// keep using [`FetchLimits::default`].
    pub fn token_metadata(max_bytes: usize, total_timeout: Duration) -> Self {
        Self {
            max_bytes,
            total_timeout,
            allow_http: true,
            ..Self::default()
        }
    }

    /// Test-only: allow plain `http://` so a loopback test server is reachable.
    #[cfg(test)]
    pub(crate) fn allow_plain_http(mut self) -> Self {
        self.allow_http = true;
        self
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

/// Why [`GuardedDns`] refused a lookup. Carried through reqwest's error chain
/// and recovered by [`classify_send_error`].
#[derive(Debug)]
enum DnsRefusal {
    /// At least one answer is in a blocked range.
    Blocked,
    /// The lookup failed or returned no address.
    Failed,
}

impl fmt::Display for DnsRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked => f.write_str("resolved address is blocked"),
            Self::Failed => f.write_str("dns resolution failed"),
        }
    }
}

impl StdError for DnsRefusal {}

/// DNS resolver plugged into the shared client: resolves through a
/// [`HostResolver`], rejects the host if ANY answer is blocked (a mixed answer
/// set is the classic rebinding trick) and returns only validated addresses.
struct GuardedDns {
    inner: Arc<dyn HostResolver>,
    is_blocked: fn(&IpAddr) -> bool,
}

impl Resolve for GuardedDns {
    fn resolve(&self, name: Name) -> Resolving {
        let inner = Arc::clone(&self.inner);
        let is_blocked = self.is_blocked;
        let host = name.as_str().to_owned();
        Box::pin(async move {
            // reqwest substitutes the URL's port for the placeholder 0.
            let addrs = inner
                .resolve(&host, 0)
                .await
                .map_err(|_| DnsRefusal::Failed)?;
            if addrs.is_empty() {
                return Err(DnsRefusal::Failed.into());
            }
            if addrs.iter().any(|a| is_blocked(&a.ip())) {
                return Err(DnsRefusal::Blocked.into());
            }
            let validated: Addrs = Box::new(addrs.into_iter());
            Ok(validated)
        })
    }
}

/// The guarded HTTP fetcher: a cached client plus the IP policy used for
/// literal-IP hosts.
pub(super) struct Fetcher {
    client: reqwest::Client,
    is_blocked: fn(&IpAddr) -> bool,
}

/// Production fetcher, built on first use (client construction loads the TLS
/// root store once, not per request or per hop).
static PRODUCTION_FETCHER: LazyLock<Result<Fetcher, String>> = LazyLock::new(|| {
    Fetcher::new(
        Arc::new(SystemResolver),
        is_private_ip,
        FETCH_CONNECT_TIMEOUT,
    )
    .map_err(|e| e.to_string())
});

/// Fetch a user-supplied URL with SSRF protection.
///
/// Policy (see `docs/eng-16792-image-guard-ssrf-hardening.md`):
/// * https only (unless the limits come from [`FetchLimits::token_metadata`]),
///   no URL credentials;
/// * names are resolved by [`GuardedDns`]: the host is rejected if ANY address
///   is private per [`is_private_ip`], and the connection uses exactly the
///   validated addresses. TLS SNI and certificate verification still use the
///   host name;
/// * redirects are followed manually (at most `limits.max_redirects`) because
///   automatic redirects would skip validation of every hop. Each hop repeats
///   scheme, credential, host-name, literal-IP and DNS validation. Manual
///   handling was chosen over "no redirects" because IPFS gateways and CDNs
///   legitimately redirect;
/// * proxies from the environment are ignored (a proxy would resolve names
///   itself and defeat validation);
/// * the body is streamed and aborted past `limits.max_bytes`;
/// * `limits.total_timeout` bounds the entire operation.
pub async fn fetch_public_bytes(url: &str, limits: &FetchLimits) -> Result<FetchOutcome, LibError> {
    match &*PRODUCTION_FETCHER {
        Ok(fetcher) => fetcher.fetch(url, limits).await,
        Err(reason) => Err(LibError::UpstreamFetch(format!(
            "http client unavailable: {reason}"
        ))),
    }
}

impl Fetcher {
    /// Build a fetcher. The resolver and IP policy are injectable so the tests
    /// can run against a loopback server; production input never reaches this
    /// constructor (it is crate-private and only [`PRODUCTION_FETCHER`] and
    /// tests call it).
    pub(super) fn new(
        resolver: Arc<dyn HostResolver>,
        is_blocked: fn(&IpAddr) -> bool,
        connect_timeout: Duration,
    ) -> Result<Self, LibError> {
        let client = reqwest::Client::builder()
            .redirect(redirect::Policy::none())
            .no_proxy()
            .connect_timeout(connect_timeout)
            .pool_max_idle_per_host(1)
            .pool_idle_timeout(Duration::from_secs(10))
            .dns_resolver(Arc::new(GuardedDns {
                inner: resolver,
                is_blocked,
            }))
            .build()
            .map_err(|e| LibError::UpstreamFetch(format!("client build failed: {e}")))?;
        Ok(Self { client, is_blocked })
    }

    /// Fetch `url` within `limits.total_timeout`.
    pub(super) async fn fetch(
        &self,
        url: &str,
        limits: &FetchLimits,
    ) -> Result<FetchOutcome, LibError> {
        tokio::time::timeout(limits.total_timeout, self.follow(url, limits))
            .await
            .map_err(|_| LibError::UpstreamFetch("fetch exceeded total timeout".into()))?
    }

    /// Issue the request and follow validated redirects up to the limit.
    async fn follow(&self, url: &str, limits: &FetchLimits) -> Result<FetchOutcome, LibError> {
        let mut current =
            Url::parse(url).map_err(|_| LibError::InvalidInput("unparseable url".into()))?;
        self.check_url(&current, limits)?;

        for hop in 0..=limits.max_redirects {
            let response = self
                .client
                .get(current.clone())
                .send()
                .await
                .map_err(|e| classify_send_error(e, &current))?;

            match self.next_redirect(&current, &response, hop, limits)? {
                Some(next) => current = next,
                None => return finish(response, limits).await,
            }
        }
        Err(LibError::UpstreamFetch("too many redirects".into()))
    }

    /// Synchronous per-hop policy: scheme, credentials, host-name denylist and,
    /// for literal-IP hosts (which bypass the DNS resolver), the IP policy.
    fn check_url(&self, url: &Url, limits: &FetchLimits) -> Result<(), LibError> {
        validate_fetch_url(url, limits)?;
        match literal_ip(url) {
            Some(ip) if (self.is_blocked)(&ip) => {
                Err(LibError::SsrfBlocked(redact_url(url.as_str())))
            }
            _ => Ok(()),
        }
    }

    /// The validated redirect target, or `None` when `response` is not a
    /// redirect that carries a `Location`.
    fn next_redirect(
        &self,
        current: &Url,
        response: &Response,
        hop: usize,
        limits: &FetchLimits,
    ) -> Result<Option<Url>, LibError> {
        if !response.status().is_redirection() {
            return Ok(None);
        }
        let Some(location) = response.headers().get(LOCATION) else {
            return Ok(None);
        };
        if hop >= limits.max_redirects {
            return Err(LibError::UpstreamFetch("too many redirects".into()));
        }
        let target = join_location(current, location)?;
        self.check_url(&target, limits)?;
        Ok(Some(target))
    }
}

/// Resolve a `Location` header (absolute, relative or scheme-relative) against
/// the current URL.
fn join_location(current: &Url, location: &HeaderValue) -> Result<Url, LibError> {
    location
        .to_str()
        .ok()
        .and_then(|loc| current.join(loc).ok())
        .ok_or_else(|| LibError::UpstreamFetch("invalid redirect location".into()))
}

/// Turn a final (non-redirect) response into the outcome.
async fn finish(response: Response, limits: &FetchLimits) -> Result<FetchOutcome, LibError> {
    let status = response.status();
    if status != StatusCode::OK {
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
    read_capped_body(response, limits.max_bytes)
        .await
        .map(FetchOutcome::Body)
}

/// Stream the body, aborting as soon as it exceeds `max_bytes`. The buffer is
/// pre-sized from the advertised length (bounded by the cap) to avoid repeated
/// growth for large images.
async fn read_capped_body(mut response: Response, max_bytes: usize) -> Result<Vec<u8>, LibError> {
    let advertised = response
        .content_length()
        .map_or(0, |len| usize::try_from(len).unwrap_or(max_bytes));
    let mut body: Vec<u8> = Vec::with_capacity(advertised.min(max_bytes));
    while let Some(chunk) = response.chunk().await.map_err(classify_body_error)? {
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(LibError::PayloadTooLarge(format!(
                "body exceeds {max_bytes} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Map a failure from `send()`. A [`DnsRefusal`] raised by [`GuardedDns`] is
/// found in the error's source chain. Messages are fixed strings: reqwest's own
/// text embeds the request URL, which must not reach logs or responses.
fn classify_send_error(err: reqwest::Error, url: &Url) -> LibError {
    let mut source: Option<&(dyn StdError + 'static)> = Some(&err);
    while let Some(cause) = source {
        if let Some(refusal) = cause.downcast_ref::<DnsRefusal>() {
            return match refusal {
                DnsRefusal::Blocked => LibError::SsrfBlocked(redact_url(url.as_str())),
                DnsRefusal::Failed => LibError::UpstreamFetch("dns resolution failed".into()),
            };
        }
        source = cause.source();
    }
    let (timed_out, connect) = (err.is_timeout(), err.is_connect());
    debug!(
        "fetch of {} failed: {}",
        redact_url(url.as_str()),
        err.without_url()
    );
    if timed_out {
        LibError::UpstreamFetch("request timed out".into())
    } else if connect {
        LibError::UpstreamFetch("connection failed".into())
    } else {
        LibError::UpstreamFetch("request failed".into())
    }
}

/// Map a failure while reading the body.
fn classify_body_error(err: reqwest::Error) -> LibError {
    let timed_out = err.is_timeout();
    debug!("reading response body failed: {}", err.without_url());
    if timed_out {
        LibError::UpstreamFetch("request timed out".into())
    } else {
        LibError::UpstreamFetch("response body read failed".into())
    }
}

/// Synchronous, network-free part of the fetch policy: scheme (https only unless
/// the limits allow http), no URL credentials, host-name denylist and
/// literal-IP checks. Callers can run it up front to reject obviously bad URLs
/// before doing any other work (e.g. a database lookup); every redirect hop is
/// checked again during the fetch.
pub fn validate_fetch_url(url: &Url, limits: &FetchLimits) -> Result<(), LibError> {
    let scheme_ok = url.scheme() == "https" || (limits.allow_http && url.scheme() == "http");
    if !scheme_ok || !url.username().is_empty() || url.password().is_some() {
        return Err(LibError::SsrfBlocked(redact_url(url.as_str())));
    }
    validate_parsed_url(url)
}

#[cfg(test)]
mod tests;
