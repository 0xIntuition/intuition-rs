# Consumer API

The consumer API can be used to re-fetch atoms, either to conform to a new schema or to retry atoms that failed to resolve

## Environment Variables

- `CONSUMER_API_PORT`: The port for the consumer API
- `RESOLVER_STREAM`: The name of the resolver Redis stream
- `REDIS_URL`: The URL of the Redis server

## Endpoints

- `/refetch_atoms`: Enqueue the atoms to be re-fetched in the resolver consumer

### Swagger UI

- `https://localhost:3000/swagger-ui/`: Swagger UI for the API 

## Example

```bash
curl --location 'http://localhost:3003/refetch_atoms' \
--header 'Content-Type: application/json' \
--data '{
    "AtomIds": ["1", "2", "3", "4", "8", "9"] 
}'
```

