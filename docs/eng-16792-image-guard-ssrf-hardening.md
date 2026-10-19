# ENG-16792: image-guard outbound fetch hardening (intuition-rs part)

Status: implemented.

Scope: `apps/image-guard` and `apps/shared-utils`. Deployment-side changes are
tracked separately. Release notes: `release_notes/` only
holds hasura-migrations, consumer and chart-api notes; there is no image-guard
convention, so none were added. Version bumped 2.0.2 -> 2.1.0 (security fix).

## Hardening areas
1. `Image::download()` validated the URL once and then relied on reqwest's default
   redirect policy, so redirect targets were not re-validated.
2. Validation did not look at what a hostname resolves to.
3. No timeout, no response size cap, plain http allowed.
4. `is_private_ip` missed v4-mapped v6, 0/8, 192.0.0/24, 198.18/15, 240/4, multicast,
   NAT64, 6to4, 2001:db8::/32.
5. `ApiError` returned every error as 500 with `to_string()` (leaks internals/URL).
6. `/swagger-ui` + `/api-docs` always mounted.

## Decisions
- **Entry point**: `shared_utils::ssrf::fetch_public_bytes(url, &FetchLimits) -> Result<FetchOutcome, LibError>`.
  `Image::download()` returns `Result<Vec<u8>, LibError>` (data URLs still decoded locally,
  exactly as the Hasura `uploadImage` action needs).
- **http**: https only by default. `FetchLimits::allow_http` is crate-private: nothing outside
  `shared-utils` can set it. The only other caller of the fetcher is the consumer's CAIP-22
  resolver (token metadata, see below), which gets the single explicitly named constructor
  `FetchLimits::token_metadata(max_bytes, total_timeout)` that permits plain http, because token
  URIs have always included plain-http metadata servers. Tests use a `#[cfg(test)]` helper.
  `Image::download_image_classify_and_store` (consumer) talks to image-guard itself and does not
  use the fetcher.
- **One shared client**: a single cached `reqwest::Client` (`LazyLock`, built on first use) with
  `redirect::Policy::none()`, `no_proxy()`, a connect timeout and pooled connections. Name
  resolution goes through `GuardedDns` (`reqwest::dns::Resolve`): it resolves via tokio
  `lookup_host`, rejects the host if ANY address is blocked and returns only the validated
  addresses, so the connector uses exactly the validated answer (no gap between a validation
  lookup and the connect-time lookup). TLS SNI/cert checks keep using the host name. The resolver
  and IP policy are injectable (`Fetcher::new`, crate-private) for tests only; production uses the
  system resolver and `is_private_ip`. Literal-IP hosts never reach a DNS resolver, so they are
  checked explicitly on every hop. Env proxies are disabled (a proxy would re-resolve names).
- **Redirects**: manual following, max 3 hops, full validation (scheme, credentials, host-name
  denylist, literal IP, DNS) on every hop. Chosen over "no redirects" because IPFS gateways and
  CDNs legitimately redirect.
- **Host-name denylist** is applied to the normalised host (trailing root dots trimmed, ASCII
  lower-cased) so `localhost.` and `x.internal.` match.
- **Timeouts**: connect 5s (a property of the shared client), total 20s per fetch
  (`FETCH_CONNECT_TIMEOUT`, `FETCH_TOTAL_TIMEOUT`). The total deadline is a single outer
  `tokio::time::timeout` covering DNS, every hop and the body; there is deliberately no second
  per-request timeout.
- **Size**: stream chunks, abort past `MAX_IMAGE_SIZE` (50 MiB); early reject on Content-Length;
  the buffer is pre-sized from `min(content-length, cap)`.
- **Concurrency**: `/upload_image_from_url` holds a slot (semaphore in `AppState`) for the whole
  request. `UPLOAD_FROM_URL_MAX_CONCURRENCY` (default 4, minimum 1) bounds how many full-size
  images can be buffered at once. A request that cannot get a slot within 30s gets `503 service
  busy` instead of queueing without bound.
- **Behaviour changes** (call out in release notes): plain `http://` image URLs now return 400;
  a non-200 origin returns 502 instead of `200 []`; a 200 origin with an empty body returns 400
  (`validate_image_bytes` rejects it as not an image); responses over the cap return 413.
- **Error variants**: the user-URL fetch path has its own `LibError::UpstreamFetch(String)`
  (fixed messages only; reqwest text embeds the request URL so it is never forwarded). The
  generic `NetworkError`/`TimeoutError`/`Reqwest` variants are also produced by the IPFS/Pinata
  upload path, so they map to a different label.
- **Logging**: every place a user URL is logged, and every `SsrfBlocked` payload, uses
  `shared_utils::ssrf::redact_url` (scheme, host, port, path only: no userinfo, query or
  fragment; non-http(s) URLs such as `data:` become a placeholder).
- **Consumer handling of image-guard errors**: `IpfsUploadMessage::process` calls
  `Image::download_image_classify_and_store`; on a non-2xx answer (now 400/413/502/503 in addition
  to 500) that function returns an error, and `process` logs a warning and returns `Ok(())`. The
  message is acknowledged and never re-queued, so a permanent 400 cannot cause an endless retry
  loop; no consumer change was needed. The trade-off is that a transient 502/503 is not retried
  either (it was not before: any error was already swallowed).
- **Pre-check before DB**: `Image::precheck_remote_url` runs the network-free checks before the
  cache lookup so blocked URLs never touch Postgres.
- **Error mapping** (`ApiError::client_response`, exhaustive match): 400 `invalid request`
  (invalid input, SSRF-blocked, not an image, bad data URL), 413 `payload too large`, 502
  `upstream fetch failed` (the user-URL fetch: DNS, connect, timeout, non-200), 502
  `upstream request failed` (our own dependencies: IPFS node, Pinata; chosen over 500 because the
  failure is a dependency's, not a bug in the service), 503 `service busy`, 500 `internal error`
  for everything else including `ExternalService` (IPFS/HF/classifier). Details are logged (`warn`
  for 4xx, `error` with Debug for 5xx).
- **Consumer CAIP-22 resolver** (`apps/consumer/src/mode/resolver/caip22_resolver.rs`): both
  token-metadata fetch sites now use `fetch_public_bytes` with 1 MiB / 30s limits and
  `FetchLimits::token_metadata`, replacing "host-name check + default client" (which followed
  redirects and resolved names unchecked). Behaviour change: a non-200 answer is an error rather
  than being parsed as metadata (a 404 JSON error body used to be accepted and stored as the
  atom's metadata). The live-network `test_fetch_real_nft_base_sepolia` depended on exactly that,
  so it is `#[ignore]`d like its sibling. Redirect/DNS blocking cannot be exercised at that call
  site without a network (the loopback test server is blocked by design); it is covered in
  `shared-utils` where the resolver is injectable, and the consumer tests assert that the real
  entry point refuses trailing-dot names, credentials and special-use literals.
- **Swagger**: `ENABLE_SWAGGER=true` (Env `enable_swagger`, default false). `/metrics` stays
  mounted for in-cluster Prometheus; it must not be exposed on a public ingress (README).
- Not restricted: destination ports (any port on a public host is allowed); considered
  over-restrictive for gateways. Public hosts only, so impact is limited to the public internet.
- Release notes: not added (only hasura-migrations / consumer / chart-api notes exist).

## `is_private_ip` additions
v4-mapped/compatible v6 (judged by the embedded v4), 0.0.0.0/8, 192.0.0.0/24, 192.88.99.0/24,
198.18.0.0/15, 224.0.0.0/4, 240.0.0.0/4, ff00::/8, NAT64 64:ff9b::/96 (embedded v4) and
64:ff9b:1::/48, 6to4 2002::/16 (embedded v4), IETF protocol assignments 2001::/23 (includes Teredo 2001::/32), 5f00::/16, 2001:db8::/32, 3fff::/20, fec0::/10,
100::/64. Also fixed an existing gap: `validate_url_not_internal` parsed `host_str()` (which keeps
brackets) so literal IPv6 hosts like `http://[fe80::1]/` were never IP-checked; it now uses
`Url::host()`.

## What each test proves
shared-utils `ssrf::fetch::tests` (loopback server, fake resolver, production policy minus loopback);
every error assertion pins the exact `LibError` variant (and message where it is a fixed string):
- guarded resolution: a name known only to the fake resolver is reachable and resolved exactly once.
- redirects to 169.254.169.254, to a hostname resolving to 10/8, to `file://`, protocol-relative
  `//10.0.0.1/`, and to a URL with userinfo are all `SsrfBlocked` (payload redacted); hit counters
  show the target was never contacted. https -> http is rejected with default limits (exercised on
  the per-hop code path, since the loopback server cannot speak TLS).
- redirect chain of 3 followed; a loop stops after exactly 4 requests (`UpstreamFetch`); a 3xx
  without `Location` is reported as its status.
- mixed public+private DNS answer rejected without connecting; empty answer and NXDOMAIN are
  `UpstreamFetch("dns resolution failed")`; names resolving to 127.0.0.1, ::ffff:127.0.0.1,
  ::ffff:169.254.169.254 are blocked with the real policy; literal-IP hosts are checked by the
  injected policy with zero resolver calls.
- http, userinfo and ftp rejected before any DNS call.
- Content-Length over cap rejected while the origin stalls; endless chunked body and a fast
  slow-drip body aborted at the cap; a slow drip below the cap hits the 300ms deadline; cap
  boundary (1000 ok, 1001 rejected); 0-byte body returned empty; Content-Length smaller than the
  body yields only the declared bytes; Content-Length larger than the body then close is
  `UpstreamFetch("response body read failed")`; connection refused; stalled origin -> deadline.
  Timing tests that expect a timeout use 300ms limits; tests that expect another outcome use a
  generous 5s limit and assert the variant (a regression would surface as the timeout variant).
`ssrf::tests`: every range, public edges (guards against over-blocking), bracketed v6 literals,
trailing-dot / mixed-case host names, redacted error payloads.
image-guard: `error::tests` (status/message table incl. the two 502 labels and 503, exact bodies),
`app::tests` (swagger 404 vs 200; full app: swagger hidden, `/metrics` 200, blocked URLs -> 400
exact `invalid request` with a dead DB proving no DB hit, dead-DB internal error -> exactly
`internal error`; concurrency limit through the real handler: held slot -> 503 after queueing,
freed slot -> served, slot released), `endpoints::tests` (empty/truncated body -> 400).
consumer `caip22_resolver::tests`: bypass forms refused at the real fetch entry point, error mapping.
integration-tests `hasura-actions-metadata.test.ts` (`pnpm run test:metadata`, run in CI by
`.github/workflows/metadata-guard.yml`): allow-list of anonymous actions, pinning actions exactly
{frontend, partner}, a synthetic templated-handler action granting anonymous fails the guard, and a
self-check that the role parser reads a known action.

## Evidence
Pre-fix (baseline 183b505 with the new tests copied in): `ssrf::tests::{blocks_ipv4_mapped_and_compatible_ipv6,
blocks_nat64_and_6to4_embedding_private_v4, blocks_reserved_special_use_and_multicast_ranges,
validate_blocks_bracketed_ipv6_literals_and_mapped_forms}` FAILED; `error::tests::blocked_url_is_400_and_leaks_nothing`
FAILED (500 + URL in body); `range_edges_do_not_over_block_public_space` passed (expected). The fetch
tests exercise the new API and the swagger test the new `with_swagger`, so they cannot compile pre-fix;
their teeth were shown by sabotage instead (each confirmed via `git diff --stat`, then restored):
re-enabling auto redirects (3 fail), removing DNS pinning (9 fail), dropping Content-Length check (1),
dropping streaming cap (1), removing v4-mapped handling (3), skipping resolved-IP check (3), removing
both timeouts (stall test fails), swagger always mounted (2), leaky 500 (4), dropping the pre-DB
precheck (1), adding `anonymous` to pinThing (2) and an extra role on uploadJsonToIpfs (1).

## Review follow-up evidence (sabotage, each confirmed to have landed before the run)
- host normalisation removed -> `blocks_hostnames_with_trailing_dot_and_mixed_case` fails
  (`http://localhost./x must be blocked`).
- `GuardedDns` no longer rejecting blocked answers -> 3 tests fail (mixed record set, redirect to a
  hostname resolving private, production-policy v6/loopback names).
- consumer fetch swapped for an unguarded `reqwest::get` -> 4 call-site tests fail.
- an extra `anonymous` role on `uploadImageFromUrl` -> the allow-list and exact-roles tests fail.

## Commands
- `cargo +1.89.0 test -p image-guard -p shared-utils -p consumer`
- `cd integration-tests && pnpm run test:metadata` (static, no stack needed)
- clippy `-D warnings` clean for image-guard and shared-utils; consumer has pre-existing lint
  failures in files this change does not touch (counts before/after in the PR description).
