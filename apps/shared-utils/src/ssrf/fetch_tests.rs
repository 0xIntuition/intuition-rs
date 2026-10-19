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
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
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
    Body(Vec<u8>),
    Status(u16),
    /// Advertises a huge Content-Length, sends 2 bytes, then stalls.
    HugeContentLength,
    /// `Transfer-Encoding: chunked`, no length, never ends.
    EndlessChunked,
    /// Sends headers and 2 of 10 body bytes, then stalls.
    Stall,
}

struct TestServer {
    port: u16,
    hits: Arc<AtomicUsize>,
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
                match route(&path) {
                    Reply::Redirect(to) => {
                        let _ = sock
                            .write_all(
                                format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                                    .as_bytes(),
                            )
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
                        let _ = sock
                            .write_all(
                                format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                                    .as_bytes(),
                            )
                            .await;
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
                        loop {
                            if sock.write_all(chunk.as_bytes()).await.is_err() {
                                return;
                            }
                        }
                    }
                    Reply::Stall => {
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab")
                            .await;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            });
        }
    });
    TestServer { port, hits }
}

fn test_limits() -> FetchLimits {
    FetchLimits {
        max_bytes: 1000,
        connect_timeout: Duration::from_secs(2),
        total_timeout: Duration::from_secs(5),
        max_redirects: 3,
        allow_http: true,
    }
}

async fn run(url: &str, limits: &FetchLimits, r: &FakeResolver) -> Result<FetchOutcome, LibError> {
    fetch_with(url, limits, r, allow_loopback_only).await
}

#[tokio::test]
async fn fetches_body_from_pinned_validated_address() {
    let srv = serve(|_| Reply::Body(b"hello".to_vec())).await;
    // `public.test` only exists in the fake resolver: success proves the
    // connection was pinned to the validated address (no second, real lookup).
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let out = run(
        &format!("http://public.test:{}/x.png", srv.port),
        &test_limits(),
        &r,
    )
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
    let srv = serve(|_| Reply::Redirect("http://169.254.169.254/latest/meta-data/".into())).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let err = run(
        &format!("http://public.test:{}/r", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::SsrfBlocked(_)), "{err:?}");
    assert_eq!(srv.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn redirect_to_hostname_resolving_private_is_blocked() {
    let srv = serve(|_| Reply::Redirect("http://rebind.test:1/secret".into())).await;
    let r = FakeResolver::default()
        .with("public.test", &["127.0.0.1"])
        .with("rebind.test", &["10.1.2.3"]);
    let err = run(
        &format!("http://public.test:{}/r", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::SsrfBlocked(_)), "{err:?}");
    // Both hops were resolved (the second one is what got it rejected).
    assert_eq!(r.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn redirect_downgrading_to_blocked_scheme_is_blocked() {
    let srv = serve(|_| Reply::Redirect("file:///etc/passwd".into())).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let err = run(
        &format!("http://public.test:{}/r", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::SsrfBlocked(_)), "{err:?}");
}

#[tokio::test]
async fn follows_public_redirect_chain_within_limit_and_stops_past_it() {
    // /a -> /b -> /c -> /d (3 hops, allowed) ; /loop redirects forever.
    let srv = serve(|p| match p {
        "/a" => Reply::Redirect("/b".into()),
        "/b" => Reply::Redirect("/c".into()),
        "/c" => Reply::Redirect("/d".into()),
        "/d" => Reply::Body(b"done".to_vec()),
        _ => Reply::Redirect("/loop".into()),
    })
    .await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let ok = run(
        &format!("http://public.test:{}/a", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap();
    assert!(matches!(ok, FetchOutcome::Body(b) if b == b"done"));

    let before = srv.hits.load(Ordering::SeqCst);
    let err = run(
        &format!("http://public.test:{}/loop", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::NetworkError(_)), "{err:?}");
    // initial request + 3 followed hops = 4 requests, no more.
    assert_eq!(srv.hits.load(Ordering::SeqCst) - before, 4);
}

#[tokio::test]
async fn any_private_answer_in_a_mixed_record_set_rejects_the_host() {
    let srv = serve(|_| Reply::Body(b"x".to_vec())).await;
    let r = FakeResolver::default().with("mixed.test", &["127.0.0.1", "192.168.1.10"]);
    let err = run(
        &format!("http://mixed.test:{}/", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::SsrfBlocked(_)), "{err:?}");
    assert_eq!(srv.hits.load(Ordering::SeqCst), 0, "must not even connect");
}

#[tokio::test]
async fn hostname_resolving_to_mapped_v6_loopback_is_blocked_with_production_policy() {
    // Production policy (not the loopback-relaxed one): nip.io style names that
    // resolve to 127.0.0.1 / ::ffff:127.0.0.1 / metadata must be rejected.
    let r = FakeResolver::default()
        .with("a.test", &["127.0.0.1"])
        .with("b.test", &["::ffff:127.0.0.1"])
        .with("c.test", &["::ffff:169.254.169.254"]);
    for host in ["a.test", "b.test", "c.test"] {
        let err = fetch_with(
            &format!("https://{host}/x.png"),
            &FetchLimits::default(),
            &r,
            is_private_ip,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LibError::SsrfBlocked(_)), "{host}: {err:?}");
    }
}

#[tokio::test]
async fn http_and_credentials_rejected_by_default_without_network() {
    let r = FakeResolver::default().with("example.com", &["93.184.216.34"]);
    for url in [
        "http://example.com/a.png",
        "https://user:pw@example.com/a.png",
        "ftp://example.com/a.png",
    ] {
        let err = fetch_with(url, &FetchLimits::default(), &r, is_private_ip)
            .await
            .unwrap_err();
        assert!(matches!(err, LibError::SsrfBlocked(_)), "{url}: {err:?}");
    }
    assert_eq!(r.calls.load(Ordering::SeqCst), 0, "rejected before DNS");
    assert!(!FetchLimits::default().allow_http);
}

#[tokio::test]
async fn unresolvable_host_is_a_network_error_not_a_panic() {
    let r = FakeResolver::default();
    let err = run("http://nope.test:1/", &test_limits(), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, LibError::NetworkError(_)), "{err:?}");
}

#[tokio::test]
async fn content_length_above_cap_is_rejected_before_reading_body() {
    let srv = serve(|_| Reply::HugeContentLength).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let started = Instant::now();
    let err = run(
        &format!("http://public.test:{}/big", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::PayloadTooLarge(_)), "{err:?}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "must not wait for the body"
    );
}

#[tokio::test]
async fn chunked_body_without_length_is_aborted_at_cap() {
    let srv = serve(|_| Reply::EndlessChunked).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let started = Instant::now();
    let err = run(
        &format!("http://public.test:{}/inf", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::PayloadTooLarge(_)), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn body_exactly_at_cap_is_accepted_and_one_over_is_not() {
    let srv = serve(|p| match p {
        "/at" => Reply::Body(vec![7u8; 1000]),
        _ => Reply::Body(vec![7u8; 1001]),
    })
    .await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let ok = run(
        &format!("http://public.test:{}/at", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap();
    assert!(matches!(ok, FetchOutcome::Body(b) if b.len() == 1000));
    let err = run(
        &format!("http://public.test:{}/over", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::PayloadTooLarge(_)), "{err:?}");
}

#[tokio::test]
async fn stalled_origin_hits_the_total_timeout() {
    let srv = serve(|_| Reply::Stall).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let mut limits = test_limits();
    limits.total_timeout = Duration::from_millis(400);
    let started = Instant::now();
    let err = run(
        &format!("http://public.test:{}/slow", srv.port),
        &limits,
        &r,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, LibError::TimeoutError(_)), "{err:?}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn non_200_is_reported_as_unexpected_status() {
    let srv = serve(|_| Reply::Status(404)).await;
    let r = FakeResolver::default().with("public.test", &["127.0.0.1"]);
    let out = run(
        &format!("http://public.test:{}/missing", srv.port),
        &test_limits(),
        &r,
    )
    .await
    .unwrap();
    assert!(matches!(out, FetchOutcome::UnexpectedStatus(404)));
}
