//! Behavioural tests for [`fetch_public_bytes`]'s validation/redirect/limit logic.
//!
//! Loopback is blocked by design, so these tests drive the crate-private
//! `fetch_with` with (a) a fake resolver that maps public-looking names onto a
//! loopback test server and (b) an IP policy that is the production
//! `is_private_ip` minus loopback. Everything else (scheme checks, redirect
//! re-validation, pinning, caps, timeouts) is the production code path.

use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Production policy except loopback, so the test server is reachable.
fn allow_loopback_only(ip: &IpAddr) -> bool {
    !ip.is_loopback() && is_private_ip(ip)
}

fn block_everything(_: &IpAddr) -> bool {
    true
}

#[derive(Default)]
struct FakeResolver {
    answers: HashMap<String, Vec<IpAddr>>,
    calls: AtomicUsize,
}

impl FakeResolver {
    fn with(mut self, host: &str, ips: &[&str]) -> Self {
        self.answers.insert(
            host.into(),
            ips.iter().map(|i| i.parse().unwrap()).collect(),
        );
        self
    }
}

impl HostResolver for FakeResolver {
    fn resolve<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            self.answers
                .get(host)
                .map(|ips| ips.iter().map(|ip| SocketAddr::new(*ip, port)).collect())
                .ok_or_else(|| std::io::Error::other("NXDOMAIN"))
        })
    }
}

enum Reply {
    Redirect(String),
    /// A 302 with no `Location` header.
    RedirectWithoutLocation,
    Body(Vec<u8>),
    Status(u16),
    /// Advertises a huge Content-Length, sends 2 bytes, then stalls.
    HugeContentLength,
    /// `Transfer-Encoding: chunked`, no length, never ends.
    EndlessChunked,
    /// Sends headers and 2 of 10 body bytes, then stalls.
    Stall,
    /// Declares `Content-Length: declared` but writes `body` and closes.
    RawLength {
        declared: usize,
        body: Vec<u8>,
    },
    /// Chunked body of `chunk` bytes every `interval_ms`, never ends.
    Drip {
        chunk: usize,
        interval_ms: u64,
    },
}

struct TestServer {
    port: u16,
    hits: Arc<AtomicUsize>,
}

async fn write_reply(sock: &mut tokio::net::TcpStream, reply: Reply) {
    match reply {
        Reply::Redirect(to) => {
            let head = format!(
                "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = sock.write_all(head.as_bytes()).await;
        }
        Reply::RedirectWithoutLocation => {
            let _ = sock
                .write_all(b"HTTP/1.1 302 Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await;
        }
        Reply::Body(data) => {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                data.len()
            );
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(&data).await;
        }
        Reply::Status(code) => {
            let head =
                format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = sock.write_all(head.as_bytes()).await;
        }
        Reply::HugeContentLength => {
            let _ = sock
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\nab")
                .await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        Reply::EndlessChunked => {
            let _ = sock
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let chunk = format!("100\r\n{}\r\n", "A".repeat(256));
            while sock.write_all(chunk.as_bytes()).await.is_ok() {}
        }
        Reply::Stall => {
            let _ = sock
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab")
                .await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        Reply::RawLength { declared, body } => {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n"
            );
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            // Dropping the socket closes the connection.
        }
        Reply::Drip { chunk, interval_ms } => {
            let _ = sock
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let frame = format!("{chunk:x}\r\n{}\r\n", "D".repeat(chunk));
            while sock.write_all(frame.as_bytes()).await.is_ok() {
                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            }
        }
    }
}

async fn serve(route: impl Fn(&str) -> Reply + Send + Sync + 'static) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let route = Arc::new(route);
    let hits_srv = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            hits_srv.fetch_add(1, Ordering::SeqCst);
            let route = route.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let mut req = Vec::new();
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req);
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                write_reply(&mut sock, route(&path)).await;
            });
        }
    });
    TestServer { port, hits }
}

/// Limits for tests that expect a *non-timeout* outcome. The timeout is long
/// enough for a loaded runner, but short enough that a regression which falls
/// through to waiting would surface as the timeout variant, not a hang.
fn test_limits() -> FetchLimits {
    FetchLimits {
        max_bytes: 1000,
        total_timeout: Duration::from_secs(5),
        max_redirects: 3,
        allow_http: false,
    }
    .allow_plain_http()
}

/// Limits for tests whose expected outcome IS the timeout.
fn short_timeout_limits() -> FetchLimits {
    FetchLimits {
        total_timeout: Duration::from_millis(300),
        ..test_limits()
    }
}

fn fetcher(r: &Arc<FakeResolver>, policy: fn(&IpAddr) -> bool) -> Fetcher {
    Fetcher::new(r.clone(), policy, Duration::from_secs(2)).unwrap()
}

async fn run(
    url: &str,
    limits: &FetchLimits,
    r: &Arc<FakeResolver>,
) -> Result<FetchOutcome, LibError> {
    fetcher(r, allow_loopback_only).fetch(url, limits).await
}

fn resolver() -> Arc<FakeResolver> {
    Arc::new(FakeResolver::default().with("public.test", &["127.0.0.1"]))
}

fn url_of(srv: &TestServer, path: &str) -> String {
    format!("http://public.test:{}{path}", srv.port)
}

/// Assert `Err(LibError::UpstreamFetch(msg))` with the exact message.
#[track_caller]
fn assert_upstream(result: Result<FetchOutcome, LibError>, expected: &str) {
    match result {
        Err(LibError::UpstreamFetch(msg)) => assert_eq!(msg, expected),
        other => panic!("expected UpstreamFetch({expected:?}), got {other:?}"),
    }
}

#[track_caller]
fn assert_blocked(result: Result<FetchOutcome, LibError>) -> String {
    match result {
        Err(LibError::SsrfBlocked(payload)) => payload,
        other => panic!("expected SsrfBlocked, got {other:?}"),
    }
}

#[tokio::test]
async fn fetches_body_through_the_guarded_resolver() {
    let srv = serve(|_| Reply::Body(b"hello".to_vec())).await;
    // `public.test` only exists in the fake resolver: success proves the
    // connector used the validated answer (no second, real lookup).
    let r = resolver();
    let out = run(&url_of(&srv, "/x.png"), &test_limits(), &r)
        .await
        .unwrap();
    assert!(matches!(out, FetchOutcome::Body(b) if b == b"hello"));
    assert_eq!(
        r.calls.load(Ordering::SeqCst),
        1,
        "exactly one resolution per hop"
    );
}

#[tokio::test]
async fn redirect_to_metadata_ip_is_blocked_and_not_followed() {
    let srv =
        serve(|_| Reply::Redirect("http://169.254.169.254/latest/meta-data/?k=SECRET".into()))
            .await;
    let r = resolver();
    let payload = assert_blocked(run(&url_of(&srv, "/r"), &test_limits(), &r).await);
    assert_eq!(payload, "http://169.254.169.254/latest/meta-data/");
    assert_eq!(srv.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn redirect_to_hostname_resolving_private_is_blocked() {
    let srv = serve(|_| Reply::Redirect("http://rebind.test:1/secret".into())).await;
    let r = Arc::new(
        FakeResolver::default()
            .with("public.test", &["127.0.0.1"])
            .with("rebind.test", &["10.1.2.3"]),
    );
    assert_blocked(run(&url_of(&srv, "/r"), &test_limits(), &r).await);
    // Both hops were resolved (the second one is what got it rejected).
    assert_eq!(r.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn redirect_downgrading_to_blocked_scheme_is_blocked() {
    let srv = serve(|_| Reply::Redirect("file:///etc/passwd".into())).await;
    assert_blocked(run(&url_of(&srv, "/r"), &test_limits(), &resolver()).await);
}

#[tokio::test]
async fn redirect_from_https_to_http_is_rejected_with_default_limits() {
    // A TLS origin is not available to the loopback server, so exercise the
    // exact per-hop code path (`join_location` + `check_url`) a redirect takes.
    let f = fetcher(&resolver(), is_private_ip);
    let current = Url::parse("https://example.com/a.png").unwrap();
    let target = join_location(
        &current,
        &HeaderValue::from_static("http://example.com/b.png"),
    )
    .unwrap();
    assert_eq!(target.scheme(), "http");
    assert!(matches!(
        f.check_url(&target, &FetchLimits::default()),
        Err(LibError::SsrfBlocked(_))
    ));
    // https -> https is fine.
    let ok = join_location(&current, &HeaderValue::from_static("/c.png")).unwrap();
    assert!(f.check_url(&ok, &FetchLimits::default()).is_ok());
}

#[tokio::test]
async fn protocol_relative_redirect_to_private_ip_is_blocked() {
    let srv = serve(|_| Reply::Redirect("//10.0.0.1/x".into())).await;
    let r = resolver();
    let payload = assert_blocked(run(&url_of(&srv, "/r"), &test_limits(), &r).await);
    assert_eq!(payload, "http://10.0.0.1/x");
    assert_eq!(srv.hits.load(Ordering::SeqCst), 1, "target never contacted");
}

#[tokio::test]
async fn redirect_with_userinfo_is_blocked_and_redacted() {
    let port = Arc::new(AtomicUsize::new(0));
    let p2 = port.clone();
    let srv = serve(move |_| {
        Reply::Redirect(format!(
            "http://user:pw@public.test:{}/x?token=SECRET",
            p2.load(Ordering::SeqCst)
        ))
    })
    .await;
    port.store(srv.port as usize, Ordering::SeqCst);
    let payload = assert_blocked(run(&url_of(&srv, "/r"), &test_limits(), &resolver()).await);
    assert!(
        !payload.contains("pw") && !payload.contains("SECRET"),
        "{payload}"
    );
    assert_eq!(srv.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn follows_public_redirect_chain_within_limit() {
    // /a -> /b -> /c -> /d (3 hops, allowed)
    let srv = serve(|p| match p {
        "/a" => Reply::Redirect("/b".into()),
        "/b" => Reply::Redirect("/c".into()),
        "/c" => Reply::Redirect("/d".into()),
        _ => Reply::Body(b"done".to_vec()),
    })
    .await;
    let ok = run(&url_of(&srv, "/a"), &test_limits(), &resolver())
        .await
        .unwrap();
    assert!(matches!(ok, FetchOutcome::Body(b) if b == b"done"));
    assert_eq!(srv.hits.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn redirect_loop_stops_after_the_hop_limit() {
    let srv = serve(|_| Reply::Redirect("/loop".into())).await;
    let result = run(&url_of(&srv, "/loop"), &test_limits(), &resolver()).await;
    assert_upstream(result, "too many redirects");
    // initial request + 3 followed hops = 4 requests, no more.
    assert_eq!(srv.hits.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn redirect_without_location_is_reported_as_status() {
    let srv = serve(|_| Reply::RedirectWithoutLocation).await;
    let out = run(&url_of(&srv, "/x"), &test_limits(), &resolver())
        .await
        .unwrap();
    assert!(
        matches!(out, FetchOutcome::UnexpectedStatus(302)),
        "{out:?}"
    );
    assert_eq!(srv.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn any_private_answer_in_a_mixed_record_set_rejects_the_host() {
    let srv = serve(|_| Reply::Body(b"x".to_vec())).await;
    let r = Arc::new(FakeResolver::default().with("mixed.test", &["127.0.0.1", "192.168.1.10"]));
    let url = format!("http://mixed.test:{}/", srv.port);
    assert_blocked(run(&url, &test_limits(), &r).await);
    assert_eq!(srv.hits.load(Ordering::SeqCst), 0, "must not even connect");
}

#[tokio::test]
async fn hostname_resolving_to_mapped_v6_loopback_is_blocked_with_production_policy() {
    // Production policy (not the loopback-relaxed one): names that resolve to
    // 127.0.0.1 / ::ffff:127.0.0.1 / the metadata address must be rejected.
    let r = Arc::new(
        FakeResolver::default()
            .with("a.test", &["127.0.0.1"])
            .with("b.test", &["::ffff:127.0.0.1"])
            .with("c.test", &["::ffff:169.254.169.254"]),
    );
    let f = fetcher(&r, is_private_ip);
    for host in ["a.test", "b.test", "c.test"] {
        let result = f
            .fetch(&format!("https://{host}/x.png"), &FetchLimits::default())
            .await;
        assert_blocked(result);
    }
}

#[tokio::test]
async fn http_and_credentials_rejected_by_default_without_network() {
    let r = Arc::new(FakeResolver::default().with("example.com", &["93.184.216.34"]));
    let f = fetcher(&r, is_private_ip);
    for url in [
        "http://example.com/a.png",
        "https://user:pw@example.com/a.png",
        "ftp://example.com/a.png",
    ] {
        assert_blocked(f.fetch(url, &FetchLimits::default()).await);
    }
    assert_eq!(r.calls.load(Ordering::SeqCst), 0, "rejected before DNS");
    assert!(!FetchLimits::default().allow_http);
    assert!(FetchLimits::token_metadata(10, Duration::from_secs(1)).allow_http);
}

#[tokio::test]
async fn literal_ip_hosts_are_checked_by_the_policy_without_dns() {
    // A public literal IP is only rejected because the injected policy says so:
    // proves literal hosts hit `check_url`'s policy and never the resolver.
    let r = Arc::new(FakeResolver::default());
    let f = fetcher(&r, block_everything);
    let payload = assert_blocked(
        f.fetch("https://8.8.8.8/a.png", &FetchLimits::default())
            .await,
    );
    assert_eq!(payload, "https://8.8.8.8/a.png");
    assert_blocked(
        f.fetch("https://[2606:4700::1111]/a.png", &FetchLimits::default())
            .await,
    );
    assert_eq!(r.calls.load(Ordering::SeqCst), 0);
    // Production policy: private literal, with and without allow_http.
    let f = fetcher(&r, is_private_ip);
    assert_blocked(
        f.fetch("https://10.0.0.1/a.png", &FetchLimits::default())
            .await,
    );
    assert_blocked(f.fetch("http://10.0.0.1/a.png", &test_limits()).await);
    assert_eq!(r.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unresolvable_host_is_an_upstream_error_not_a_panic() {
    let r = Arc::new(FakeResolver::default());
    let result = run("http://nope.test:1/", &test_limits(), &r).await;
    assert_upstream(result, "dns resolution failed");
}

#[tokio::test]
async fn empty_dns_answer_is_an_upstream_error() {
    let r = Arc::new(FakeResolver::default().with("empty.test", &[]));
    let result = run("http://empty.test:1/", &test_limits(), &r).await;
    assert_upstream(result, "dns resolution failed");
    assert_eq!(r.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_url_is_invalid_input() {
    let result = run("not a url", &test_limits(), &resolver()).await;
    assert!(
        matches!(result, Err(LibError::InvalidInput(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn content_length_above_cap_is_rejected_before_reading_body() {
    let srv = serve(|_| Reply::HugeContentLength).await;
    let result = run(&url_of(&srv, "/big"), &test_limits(), &resolver()).await;
    // The origin stalls for 30s; reaching the timeout variant instead would mean
    // the body was awaited.
    assert!(
        matches!(result, Err(LibError::PayloadTooLarge(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn chunked_body_without_length_is_aborted_at_cap() {
    let srv = serve(|_| Reply::EndlessChunked).await;
    let result = run(&url_of(&srv, "/inf"), &test_limits(), &resolver()).await;
    assert!(
        matches!(result, Err(LibError::PayloadTooLarge(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn body_exactly_at_cap_is_accepted_and_one_over_is_not() {
    let srv = serve(|p| match p {
        "/at" => Reply::Body(vec![7u8; 1000]),
        _ => Reply::Body(vec![7u8; 1001]),
    })
    .await;
    let r = resolver();
    let ok = run(&url_of(&srv, "/at"), &test_limits(), &r).await.unwrap();
    assert!(matches!(ok, FetchOutcome::Body(b) if b.len() == 1000));
    let result = run(&url_of(&srv, "/over"), &test_limits(), &r).await;
    assert!(
        matches!(result, Err(LibError::PayloadTooLarge(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn zero_byte_body_is_returned_as_empty() {
    let srv = serve(|_| Reply::Body(Vec::new())).await;
    let out = run(&url_of(&srv, "/empty.png"), &test_limits(), &resolver())
        .await
        .unwrap();
    assert!(
        matches!(&out, FetchOutcome::Body(b) if b.is_empty()),
        "{out:?}"
    );
}

#[tokio::test]
async fn content_length_smaller_than_body_yields_only_the_declared_bytes() {
    let srv = serve(|_| Reply::RawLength {
        declared: 5,
        body: vec![b'z'; 50],
    })
    .await;
    let out = run(&url_of(&srv, "/x"), &test_limits(), &resolver())
        .await
        .unwrap();
    assert!(
        matches!(&out, FetchOutcome::Body(b) if b == b"zzzzz"),
        "{out:?}"
    );
}

#[tokio::test]
async fn content_length_larger_than_body_then_close_is_an_upstream_error() {
    let srv = serve(|_| Reply::RawLength {
        declared: 500,
        body: vec![b'z'; 10],
    })
    .await;
    let result = run(&url_of(&srv, "/x"), &test_limits(), &resolver()).await;
    assert_upstream(result, "response body read failed");
}

#[tokio::test]
async fn slow_drip_body_is_cut_off_at_the_cap() {
    // 400 bytes every 20ms crosses the 1000 byte cap after ~3 chunks, long
    // before the 5s deadline.
    let srv = serve(|_| Reply::Drip {
        chunk: 400,
        interval_ms: 20,
    })
    .await;
    let result = run(&url_of(&srv, "/drip"), &test_limits(), &resolver()).await;
    assert!(
        matches!(result, Err(LibError::PayloadTooLarge(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn slow_drip_below_the_cap_hits_the_total_timeout() {
    // 10 bytes per 50ms would need ~5s to reach the cap; the 300ms deadline
    // must win.
    let srv = serve(|_| Reply::Drip {
        chunk: 10,
        interval_ms: 50,
    })
    .await;
    let started = Instant::now();
    let result = run(&url_of(&srv, "/drip"), &short_timeout_limits(), &resolver()).await;
    assert_upstream(result, "fetch exceeded total timeout");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn stalled_origin_hits_the_total_timeout() {
    let srv = serve(|_| Reply::Stall).await;
    let result = run(&url_of(&srv, "/slow"), &short_timeout_limits(), &resolver()).await;
    assert_upstream(result, "fetch exceeded total timeout");
}

#[tokio::test]
async fn non_200_is_reported_as_unexpected_status() {
    let srv = serve(|_| Reply::Status(404)).await;
    let out = run(&url_of(&srv, "/missing"), &test_limits(), &resolver())
        .await
        .unwrap();
    assert!(matches!(out, FetchOutcome::UnexpectedStatus(404)));
}

#[tokio::test]
async fn connection_refused_is_an_upstream_error() {
    // Bind then drop to get a port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://public.test:{port}/x");
    let result = run(&url, &test_limits(), &resolver()).await;
    assert_upstream(result, "connection failed");
}

#[tokio::test]
async fn pooled_client_serves_repeated_redirecting_fetches() {
    // Two hops against the same origin through the same fetcher: a per-hop
    // client would still pass, but the pooled client must not break redirects
    // that reuse a connection to the same authority.
    let srv = serve(|p| match p {
        "/a" => Reply::Redirect("/b".into()),
        _ => Reply::Body(b"ok".to_vec()),
    })
    .await;
    let r = resolver();
    let f = fetcher(&r, allow_loopback_only);
    for _ in 0..2 {
        let out = f.fetch(&url_of(&srv, "/a"), &test_limits()).await.unwrap();
        assert!(matches!(out, FetchOutcome::Body(b) if b == b"ok"));
    }
}
