# Image Guard

Image Guard is a simple API that uploads images to IPFS and classifies them using the Falconsai model hosted on Hugging Face.
The image is then pinned to Pinata for persistence, and we also store the classification scores in a database.

## Environment Variables

- `IPFS_UPLOAD_URL`: The URL of the IPFS upload service
- `IPFS_FETCH_URL`: The URL of the IPFS fetch service
- `PINATA_API_JWT`: The JWT for the Pinata API
- `UPLOAD_FROM_URL_MAX_CONCURRENCY`: Maximum number of `/upload_image_from_url` requests processed at once (default `4`, minimum `1`). Each in-flight request can buffer an image of up to 50 MiB. A request that cannot get a slot within 30 seconds is answered with `503`.
- `ENABLE_SWAGGER`: Set to `true` to mount `/swagger-ui` and `/api-docs/openapi.json`. Unset/`false` by default (routes return 404).

## Endpoints

- `/upload`: Uploads an image file to IPFS and classifies it
- `/upload_image_from_url`: Downloads an image from a URL (or decodes a data URL), uploads it to IPFS, and classifies it

- `/metrics`: Prometheus metrics. **In-cluster scraping only: never expose this route on a public ingress.**

### Swagger UI (opt-in)

Disabled by default. Start with `ENABLE_SWAGGER=true` to serve
`http://localhost:3000/swagger-ui/` (local development only).

## Outbound fetch policy (`/upload_image_from_url`)

User-supplied URLs are fetched through `shared_utils::ssrf::fetch_public_bytes`:

- `https` only. `http://` (answered with `400`), `file://`, `ftp://` etc. are rejected, as are URLs with embedded credentials. `data:` URLs are decoded locally, no network.
- The hostname is resolved by the service; the request is rejected if ANY resolved
  address is private/loopback/link-local/CGNAT/reserved/multicast/documentation/special-use, or an
  IPv6 form embedding such a v4 (v4-mapped, NAT64, 6to4). The connection uses only the validated
  addresses, so a DNS answer change between check and connect cannot win. Literal-IP hosts are
  checked on every hop.
- Redirects are followed manually, at most 3 hops, and every hop repeats the full validation.
- Connect timeout 5s, total timeout 20s for the whole fetch (DNS, hops, body).
- Response bodies are streamed and aborted once they exceed `MAX_IMAGE_SIZE` (50 MiB);
  a larger `Content-Length` is rejected before reading the body.
- An origin that answers anything other than `200` is a `502`; a `200` with an empty or non-image
  body is a `400`.
- Concurrency is bounded by `UPLOAD_FROM_URL_MAX_CONCURRENCY` (see above).

## Error responses

Errors carry generic bodies and never echo the URL, resolved IPs or library errors
(details are logged server side only, with URLs reduced to scheme, host and path):

| Condition | Status | Body |
| --- | --- | --- |
| invalid input, not an image (including an empty body), blocked URL (including `http://`) | 400 | `invalid request` |
| payload too large | 413 | `payload too large` |
| fetching the image URL failed: DNS, connect, timeout, redirect limit, non-200 origin | 502 | `upstream fetch failed` |
| a backing service failed (IPFS node, Pinata) | 502 | `upstream request failed` |
| all request slots busy for 30 seconds | 503 | `service busy` |
| anything else | 500 | `internal error` |

The two 502 labels let callers tell "the image origin is broken" from "our storage backend is
broken". Callers that process queue messages (the consumer) log the failure and do not retry it.

Access control: the Hasura actions that proxy to this service are granted only to the
`frontend` and `partner` roles (guarded by `integration-tests/src/hasura-actions-metadata.test.ts`).

## Examples

### Upload a file directly

```bash
curl -X POST http://localhost:3000/upload -F "file=@path/to/image.jpg"
```

### Upload from a URL

```bash
curl -X POST http://localhost:3000/upload_image_from_url \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com/image.png"}'
```

### Upload from a data URL (base64-encoded)

```bash
curl -X POST http://localhost:3000/upload_image_from_url \
  -H "Content-Type: application/json" \
  -d '{"url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="}'
```

