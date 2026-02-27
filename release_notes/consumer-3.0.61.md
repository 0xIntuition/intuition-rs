# consumer:3.0.61

**Date:** 2026-02-27

## Fix: Resolver consumer database connection pool starvation

### Problem

The resolver consumer was experiencing database connection pool exhaustion, causing batch processing to stall (batches taking 60+ seconds instead of ~2.5s) and messages to accumulate in the Redis PEL (Pending Entry List).

**Root cause:** The `process_atom()` function opened a database transaction (`BEGIN`) to fetch an atom, then performed slow network I/O (IPFS fetches, HTTP requests) while the transaction remained open. This left connections in an `idle in transaction` state for the entire duration of external requests (potentially 30-60+ seconds per message). With 10 concurrent workers and a pool of 10 connections, all connections would become parked during slow IPFS fetches, starving new requests.

Additionally, the transaction provided no actual atomicity — sub-operations (upserts for TextObject, JsonObject, AtomValue, etc.) were already using the connection pool directly instead of the transaction handle, making the transaction purely a source of connection waste.

### Fix

Removed the unnecessary transaction from the resolver consumer's atom processing path. All network I/O (IPFS fetches, RPC calls) now runs without holding a database connection. DB connections are only acquired for individual, short-lived queries and upserts.

### Changes

- `apps/consumer/src/mode/resolver/types.rs`
  - `process_atom()` — removed transaction; network I/O and DB writes use the pool directly
  - `resolve_and_parse_atom_data()` — removed `tx` parameter, uses pool for initial atom read
  - `handle_known_atom_type()` — removed `tx` parameter
  - `find_and_update_atom()` — removed `tx` parameter, uses pool consistently
  - `mark_atom_as_failed()` — changed from transaction to pool

- `apps/consumer/src/mode/resolver/atom_resolver.rs`
  - Reduced log verbosity for non-IPFS atom data checks from `warn!` to `debug!` (these fire for every non-IPFS atom and were flooding logs, making real issues hard to spot)

### Impact

- **Scope:** Resolver consumer only. Decoded, raw, and IPFS upload consumers are unaffected.
- **Safety:** All DB writes in the resolver path are idempotent upserts, so transactional atomicity was not required.
- **Expected result:** Eliminates `idle in transaction` connection starvation, restoring normal batch processing throughput (~2.5s per batch of 100 messages).
