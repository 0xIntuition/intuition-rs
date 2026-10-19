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
  `Image::download()` now returns `Result<Vec<u8>, LibError>` (data URLs still decoded locally,
  exactly as the Hasura `uploadImage` action needs).
- **http**: https only. Workspace grep: the only caller of `Image::download()` is image-guard's
  `/upload_image_from_url` (user-supplied URLs). `Image::download_image_classify_and_store`
  (consumer) talks to image-guard itself (in-cluster http) and does not use `download()`; the
  caip22 resolver has its own fetch path and only uses `validate_url_not_internal`/`is_private_ip`
  (signatures unchanged, behaviour extended). Nobody needs http, so `FetchLimits::allow_http`
  exists as an explicit opt-in but nothing sets it outside tests.
- **DNS**: resolved with tokio `lookup_host`; any private answer rejects the host; the client is
  built per hop with `resolve_to_addrs(host, validated_addrs)` so the connect uses exactly those
  addresses (no second lookup) while TLS SNI/cert checks keep using the host name. Env proxies are
  disabled (`no_proxy`) since a proxy would re-resolve names itself.
- **Redirects**: `Policy::none()` plus manual following, max 3 hops, full validation each hop.
  Chosen over "no redirects" because IPFS gateways/CDNs legitimately redirect.
- **Timeouts**: connect 5s, total 20s (`FETCH_CONNECT_TIMEOUT`, `FETCH_TOTAL_TIMEOUT`). The total
  deadline is enforced twice (outer `tokio::time::timeout` covering DNS + all hops, and the
  reqwest client timeout per hop): sabotaging only one still passes the stall test; removing both
  fails it.
- **Size**: stream chunks, abort past `MAX_IMAGE_SIZE` (50 MiB); early reject on Content-Length.
- **Non-200 from origin**: previously `Ok(None)` -> HTTP 200 `[]` (silent success). Now
  `LibError::NetworkError` -> 502 `upstream fetch failed`. Compatibility check: Hasura
  `uploadImage`/`uploadImageFromUrl` response_transform wraps `$body` in `{"images": ...}` for 2xx
  and surfaces non-2xx as an action error (previous `[]` produced an empty list); the consumer
  caller (`download_image_classify_and_store`) already only logs a warning on error. Transforms
  were not changed.
- **Pre-check before DB**: `Image::precheck_remote_url` runs the network-free checks before the
  cache lookup so blocked URLs never touch Postgres.
- **Error mapping** (`ApiError::client_response`, exhaustive match): 400 `invalid request`
  (invalid input, SSRF-blocked, not an image, bad data URL), 413 `payload too large`, 502
  `upstream fetch failed` (network/timeout/reqwest from the fetch), 500 `internal error` for all
  else including `ExternalService` (IPFS/HF/classifier). Details are logged (`warn` for 4xx,
  `error` with Debug for 5xx).
- **Swagger**: `ENABLE_SWAGGER=true` (Env `enable_swagger`, default false). `/metrics` stays
  mounted for in-cluster Prometheus; it must not be exposed on a public ingress (README).
- Not restricted: destination ports (any port on a public host is allowed); considered
  over-restrictive for gateways. Public hosts only, so impact is limited to the public internet.
- Release notes: not added (only hasura-migrations / consumer / chart-api notes exist).

## `is_private_ip` additions
v4-mapped/compatible v6 (judged by the embedded v4), 0.0.0.0/8, 192.0.0.0/24, 192.88.99.0/24,
198.18.0.0/15, 224.0.0.0/4, 240.0.0.0/4, ff00::/8, NAT64 64:ff9b::/96 (embedded v4) and
64:ff9b:1::/48, 6to4 2002::/16 (embedded v4), Teredo 2001::/32, 2001:db8::/32, 3fff::/20, fec0::/10,
100::/64. Also fixed an existing gap: `validate_url_not_internal` parsed `host_str()` (which keeps
brackets) so literal IPv6 hosts like `http://[fe80::1]/` were never IP-checked; it now uses
`Url::host()`.

## What each test proves
shared-utils `ssrf::fetch_tests` (loopback server, fake resolver, production policy minus loopback):
- pinned success: a name known only to the fake resolver is reachable and resolved exactly once.
- redirect to 169.254.169.254, to a hostname resolving to 10/8, and to `file://` are blocked; server
  hit counter shows the redirect target was never contacted / only one request was made.
- redirect chain of 3 followed, a loop stops after exactly 4 requests.
- mixed public+private DNS answer rejected without connecting (0 server hits).
- names resolving to 127.0.0.1, ::ffff:127.0.0.1, ::ffff:169.254.169.254 blocked with the real policy.
- http, userinfo and ftp rejected before any DNS call (resolver call count 0).
- Content-Length over cap rejected quickly while the server stalls (proves early reject);
  endless chunked body aborted; cap boundary (1000 ok, 1001 rejected); stalled origin -> TimeoutError.
`ssrf::tests`: every new range, public edges (guards against over-blocking), bracketed v6 literals.
image-guard: `error::tests` (status/message table, no URL/IP/library words, exact `internal error`),
`app::tests` (swagger 404 vs 200 with a real spec body; full app: swagger hidden, `/metrics` 200,
blocked URLs -> 400 `invalid request` with a dead DB proving no DB hit, dead-DB internal error ->
exactly `internal error`).
integration-tests `hasura-actions-metadata.test.ts`: static YAML test; the six actions must have
only frontend/partner, anything proxying to image-guard/Pinata must be in the guarded list, plus a
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

## Commands
- `cargo +1.89.0 test -p image-guard -p shared-utils` (5 + 43 pass)
- `cargo +1.89.0 test -p consumer` (27 pass, 1 ignored)
- `cd integration-tests && pnpm exec vitest run src/hasura-actions-metadata.test.ts` (4 pass; static,
  no stack needed; the file is also picked up by `pnpm test` in that package)
- clippy `-D warnings` clean for image-guard and shared-utils. `--workspace` fails on pre-existing
  lints in `tui` and `consumer` (unrelated files).
