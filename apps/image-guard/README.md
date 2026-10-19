# Image Guard

Image Guard is a simple API that uploads images to IPFS and classifies them using the Falconsai model hosted on Hugging Face.
The image is then pinned to Pinata for persistence, and we also store the classification scores in a database.

## Environment Variables

- `IPFS_UPLOAD_URL`: The URL of the IPFS upload service
- `IPFS_FETCH_URL`: The URL of the IPFS fetch service
- `PINATA_API_JWT`: The JWT for the Pinata API
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

- `https` only. `http://`, `file://`, `ftp://` etc. are rejected. `data:` URLs are decoded locally, no network.
- The hostname is resolved by the service; the request is rejected if ANY resolved
  address is private/loopback/link-local/CGNAT/reserved/multicast/documentation, or an
  IPv6 form embedding such a v4 (v4-mapped, NAT64, 6to4). The connection is then pinned to
  the validated addresses, so DNS rebinding between check and connect cannot win.
- Redirects are followed manually, at most 3 hops, and every hop repeats the full validation.
- Connect timeout 5s, total timeout 20s.
- Response bodies are streamed and aborted once they exceed `MAX_IMAGE_SIZE` (50 MiB);
  a larger `Content-Length` is rejected before reading the body.

## Error responses

Errors carry generic bodies and never echo the URL, resolved IPs or library errors
(details are logged server side only):

| Condition | Status | Body |
| --- | --- | --- |
| invalid input, not an image, blocked URL | 400 | `invalid request` |
| payload too large | 413 | `payload too large` |
| upstream fetch failed / non-200 / timeout | 502 | `upstream fetch failed` |
| anything else | 500 | `internal error` |

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

