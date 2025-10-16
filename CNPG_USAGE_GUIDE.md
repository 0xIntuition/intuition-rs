# CNPG Database Connection Pool Usage Guide

This guide explains how to configure and use the database connection pool settings for CNPG (CloudNativePG) environments.

## Configuration

The database connection pool can now be configured via environment variables. Add these to your `.env` file or Kubernetes ConfigMap/Secret:

### Recommended Settings for CNPG

```bash
# CNPG-optimized settings
DATABASE_MIN_CONNECTIONS=10
DATABASE_MAX_CONNECTIONS=30
DATABASE_ACQUIRE_TIMEOUT=60
DATABASE_IDLE_TIMEOUT=300
DATABASE_MAX_LIFETIME=1800
```

### Default Settings (TimescaleDB/Standard Postgres)

```bash
# Standard settings
DATABASE_MIN_CONNECTIONS=5
DATABASE_MAX_CONNECTIONS=50
DATABASE_ACQUIRE_TIMEOUT=30
DATABASE_IDLE_TIMEOUT=600
DATABASE_MAX_LIFETIME=1800
```

## Understanding the Settings

- **DATABASE_MIN_CONNECTIONS**: Minimum connections kept alive. Higher values reduce cold starts but use more resources.
- **DATABASE_MAX_CONNECTIONS**: Maximum concurrent connections. Keep lower for CNPG to avoid overwhelming the cluster.
- **DATABASE_ACQUIRE_TIMEOUT**: Seconds to wait when getting a connection. Increase for high-latency environments.
- **DATABASE_IDLE_TIMEOUT**: Seconds before idle connections are closed. Shorter values help with connection recycling.
- **DATABASE_MAX_LIFETIME**: Maximum lifetime of any connection. Prevents stale connections.

## Using Retry Logic for Pool Timeouts

The `retry_on_pool_timeout` function automatically retries database operations that fail with pool timeout errors.

### Example 1: Simple Retry Wrapper

```rust
use shared_utils::postgres::retry_on_pool_timeout;
use models::atom::Atom;

// Wrap any database operation that might timeout
let atom = retry_on_pool_timeout(
    || async {
        Atom::find_by_id(
            atom_id,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await
    },
    3,   // max_retries: 3 attempts
    100  // initial_backoff_ms: start with 100ms, doubles each retry
).await?;
```

### Example 2: Retry in Event Handler

```rust
use shared_utils::postgres::retry_on_pool_timeout;

async fn process_event(
    &self,
    decoded_consumer_context: &DecodedConsumerContext,
    event: &DecodedMessage,
) -> Result<(), ConsumerError> {
    // Wrap the database-heavy operation with retry logic
    retry_on_pool_timeout(
        || async {
            // Get or create vault and atom
            let (vault, atom) = self.0
                .get_or_create_vault_and_atom(decoded_consumer_context, event)
                .await?;

            // Update metadata
            let metadata = get_supported_atom_metadata(
                &mut atom,
                &decoded_atom_data,
                decoded_consumer_context
            ).await?;

            Ok((vault, atom, metadata))
        },
        3,
        100
    ).await?;

    Ok(())
}
```

### Example 3: Retry with Custom Settings

```rust
// For critical operations, use more aggressive retry settings
let result = retry_on_pool_timeout(
    || async {
        triple.upsert(&schema, &pool).await
    },
    5,   // Retry up to 5 times
    50   // Start with 50ms backoff
).await?;
```

## Monitoring

The retry logic automatically logs warnings when pool timeouts occur:

```
WARN Pool timeout on attempt 1/3, retrying after 100ms...
WARN Pool timeout on attempt 2/3, retrying after 200ms...
```

Watch for these messages in your logs to identify if you need to adjust pool settings.

## Kubernetes Deployment Configuration

Add these environment variables to your Kubernetes deployment for the decoded consumer:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: intuition-testnet-cnpg-decoded-consumer
spec:
  template:
    spec:
      containers:
      - name: consumer
        env:
        - name: DATABASE_MIN_CONNECTIONS
          value: "10"
        - name: DATABASE_MAX_CONNECTIONS
          value: "30"
        - name: DATABASE_ACQUIRE_TIMEOUT
          value: "60"
        - name: DATABASE_IDLE_TIMEOUT
          value: "300"
        - name: DATABASE_MAX_LIFETIME
          value: "1800"
```

## Troubleshooting

### Still Getting PoolTimedOut Errors?

1. **Check CNPG max_connections**: Ensure your CNPG cluster's `max_connections` is higher than `DATABASE_MAX_CONNECTIONS`
2. **Check PgBouncer settings**: If using PgBouncer, ensure pool sizes are adequate
3. **Increase acquire timeout**: Try `DATABASE_ACQUIRE_TIMEOUT=90` or higher
4. **Reduce max connections**: Try `DATABASE_MAX_CONNECTIONS=20`
5. **Add retry logic**: Wrap critical database operations with `retry_on_pool_timeout`

### Checking CNPG Connection Limits

```bash
# Connect to your CNPG cluster
kubectl exec -it <cnpg-pod-name> -n intuition-testnet-cnpg -- psql -U postgres

# Check current connections
SELECT count(*) FROM pg_stat_activity;

# Check max connections
SHOW max_connections;
```

### Performance Testing

Test your configuration under load:

```bash
# Monitor pool usage
kubectl logs -f deployment/intuition-testnet-cnpg-decoded-consumer -n intuition-testnet-cnpg | grep "pool"

# Watch for timeout patterns
kubectl logs -f deployment/intuition-testnet-cnpg-decoded-consumer -n intuition-testnet-cnpg | grep "Pool timeout"
```

## Migration Path

The changes are **backward compatible**. Existing deployments will use default values if environment variables are not set.

To migrate:
1. Deploy the updated code
2. Monitor logs for pool-related messages
3. Add environment variables to your deployment
4. Restart the consumer pods
5. Monitor for improvements
