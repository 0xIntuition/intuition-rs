# Image Guard

Image Guard is a simple API that uploads images to IPFS and classifies them using the Falconsai model hosted on Hugging Face.
The image is then pinned to Pinata for persistence, and we also store the classification scores in a database.

## Environment Variables

- `IPFS_UPLOAD_URL`: The URL of the IPFS upload service
- `IPFS_FETCH_URL`: The URL of the IPFS fetch service
- `PINATA_API_JWT`: The JWT for the Pinata API

## Endpoints

- `/upload`: Uploads an image file to IPFS and classifies it
- `/upload_image_from_url`: Downloads an image from a URL (or decodes a data URL), uploads it to IPFS, and classifies it

### Swagger UI

- `https://localhost:3000/swagger-ui/`: Swagger UI for the API

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

