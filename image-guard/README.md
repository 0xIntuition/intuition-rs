# Image Guard

Image Guard is a simple API that uploads images to IPFS and classifies them using the Falconsai model hosted on Hugging Face.
The image is then pinned to Pinata for persistence, and we also store the classification scores in a database.

## Environment Variables

- `IPFS_UPLOAD_URL`: The URL of the IPFS upload service
- `IPFS_FETCH_URL`: The URL of the IPFS fetch service
- `PINATA_API_JWT`: The JWT for the Pinata API

## Endpoints

- `/upload`: Uploads an image to IPFS and classifies it

### Swagger UI

- `https://localhost:3000/swagger-ui/`: Swagger UI for the API 

## Example

```bash
curl -X POST http://localhost:3000/upload -F "file=@path/to/image.jpg"
```

