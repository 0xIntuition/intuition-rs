# CNPG Connection Pool Exhaustion - Root Cause & Solution

## Problem Summary

The decoded consumer is experiencing `SqlError(PoolTimedOut)` errors in CNPG environments but not in TimescaleDB environments.

## Root Cause

### High Connection Churn Rate

Each `AtomCreated` event performs **10-15 sequential database operations**:

```
AtomCreated Event Processing:
├── Atom::find_by_id()                    [acquire connection #1]
├── Vault::find_by_term_id_and_curve_id() [acquire connection #2]
├── Term::find_by_id()                    [acquire connection #3]
├── Term::upsert()                        [acquire connection #4]
├── Vault::insert()                       [acquire connection #5]
├── Account::find_by_id() (wallet)        [acquire connection #6]
├── Account::upsert() (wallet)            [acquire connection #7]
├── Atom::upsert() #1                     [acquire connection #8]
├── Account::find_by_id() (creator)       [acquire connection #9]
├── Account::upsert() (creator)           [acquire connection #10]
├── Atom::upsert() #2                     [acquire connection #11]
├── Account::upsert() (update atom_id)    [acquire connection #12]
├── AtomValue::find_by_id()               [acquire connection #13]
├── AtomValue::upsert()                   [acquire connection #14]
└── Event::upsert()                       [acquire connection #15]
```

Similarly, `TripleCreated` events also perform 10+ operations.

### Why CNPG Fails

1. **Network Latency**: CNPG in GKE adds ~5-10ms per connection acquisition vs local TimescaleDB
2. **Connection Acquisition Time**: 15 operations × 10ms = **150ms minimum** per event just for connection overhead
3. **Pool Saturation**: With high event throughput, all 50 connections can be busy, causing new `acquire()` calls to wait
4. **Timeout Cascade**: Once timeouts start, they cascade because events back up

### Why Transactions Were Removed

The transactions were removed (InitializeEventHandler and TripleCreatedEventHandler) because:
- They were holding connections for the **entire duration** of multi-step operations
- In CNPG, this made the problem worse by increasing connection hold time
- However, removing them exposed the underlying connection churn problem

## Solutions (In Order of Recommendation)

### Option 1: **Adjust Pool Configuration (Immediate Fix)**

Update your CNPG deployment environment variables:

```yaml
env:
  # Increase minimum to avoid cold starts
  - name: DATABASE_MIN_CONNECTIONS
    value: "15"

  # Reduce maximum to avoid overwhelming CNPG
  - name: DATABASE_MAX_CONNECTIONS
    value: "40"

  # Increase timeout for CNPG latency
  - name: DATABASE_ACQUIRE_TIMEOUT
    value: "90"

  # Faster connection recycling
  - name: DATABASE_IDLE_TIMEOUT
    value: "180"

  # Prevent stale connections
  - name: DATABASE_MAX_LIFETIME
    value: "1800"
```

**Why this helps:**
- Higher min_connections keeps warm connections ready
- Longer acquire_timeout tolerates CNPG latency
- Lower max_connections prevents overwhelming CNPG cluster

### Option 2: **Add Retry Logic to Event Handlers (Quick Win)**

Wrap the entire event processing in retry logic:

```rust
// In atom_created/event_handler.rs
async fn process_event(
    &self,
    decoded_consumer_context: &DecodedConsumerContext,
    event: &DecodedMessage,
) -> Result<(), ConsumerError> {
    use shared_utils::postgres::retry_on_pool_timeout;

    retry_on_pool_timeout(
        || async {
            // All existing logic here
            self.process_event_inner(decoded_consumer_context, event).await
        },
        3,
        100
    ).await
}
```

**Why this helps:**
- Automatically retries on pool timeout
- Exponential backoff reduces thundering herd
- Transparent to rest of codebase

### Option 3: **Use Explicit Transactions (Best Long-term Solution)**

Re-introduce transactions but with proper connection management:

```rust
async fn process_event(
    &self,
    decoded_consumer_context: &DecodedConsumerContext,
    event: &DecodedMessage,
) -> Result<(), ConsumerError> {
    use shared_utils::postgres::retry_on_pool_timeout;

    // Retry the transaction acquisition itself
    retry_on_pool_timeout(
        || async {
            let mut tx = decoded_consumer_context.pg_pool.begin().await
                .map_err(|e| ConsumerError::DatabaseError(e.to_string()))?;

            // Pass &mut tx to ALL operations
            let (vault, mut atom) = self.0
                .get_or_create_vault_and_atom_with_tx(
                    decoded_consumer_context,
                    event,
                    &mut tx
                ).await?;

            let decoded_atom_data = self.0
                .decode_atom_data_and_update_atom_with_tx(
                    &mut atom,
                    decoded_consumer_context,
                    event,
                    &mut tx
                ).await?;

            // ... rest of operations with &mut tx ...

            tx.commit().await
                .map_err(|e| ConsumerError::DatabaseError(e.to_string()))?;

            Ok(())
        },
        3,
        100
    ).await
}
```

**Why this is best:**
- **1 connection** for entire event instead of 15
- Atomic operations (all-or-nothing)
- Much lower connection churn
- Requires refactoring all upsert calls to accept transactions

### Option 4: **Check CNPG Configuration**

Verify your CNPG cluster settings:

```bash
# Check max_connections on CNPG
kubectl exec -it <cnpg-pod> -n intuition-testnet-cnpg -- \
  psql -U postgres -c "SHOW max_connections;"

# Should be at least 100 (2-3x your app's max_connections)
```

If CNPG's `max_connections` is too low:

```yaml
# In your CNPG Cluster manifest
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
spec:
  postgresql:
    parameters:
      max_connections: "200"  # Increase this
```

### Option 5: **Add Connection Pooler (PgBouncer)**

Add PgBouncer between your app and CNPG:

```yaml
apiVersion: postgresql.cnpg.io/v1
kind: Pooler
metadata:
  name: intuition-pooler
spec:
  cluster:
    name: intuition-cnpg-cluster
  instances: 3
  type: rw
  pgbouncer:
    poolMode: transaction  # Important!
    parameters:
      max_client_conn: "1000"
      default_pool_size: "25"
```

## Recommended Action Plan

1. **Immediate** (< 5 min): Update environment variables (Option 1)
2. **Short-term** (< 1 hour): Add retry logic to event handlers (Option 2)
3. **Medium-term** (< 1 day): Verify CNPG max_connections (Option 4)
4. **Long-term** (< 1 week): Refactor to use transactions (Option 3)
5. **If still issues**: Add PgBouncer (Option 5)

## Monitoring

Add these checks to your monitoring:

```bash
# Watch for pool timeout errors
kubectl logs -f deployment/intuition-testnet-cnpg-decoded-consumer \
  -n intuition-testnet-cnpg | grep "PoolTimedOut"

# Monitor connection count
kubectl exec -it <cnpg-pod> -n intuition-testnet-cnpg -- \
  psql -U postgres -c "SELECT count(*), state FROM pg_stat_activity GROUP BY state;"
```

Expected output after fixes:
```
 count | state
-------+--------
    15 | active
     5 | idle
(2 rows)
```

If you see > 50 active connections, you need to reduce `DATABASE_MAX_CONNECTIONS`.
