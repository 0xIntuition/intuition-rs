# Summary: Soft vs Hard Path in SurrealDB

## Key Ideas
- **Trust model**: The Soft Path (User → API → DB) is trusted; the Hard Path (Blockchain → Indexer → DB) is authoritative.
- **Permissions**: Use `DEFINE FIELD ... PERMISSIONS` to make critical fields read-only for normal users/API, writable only by the Indexer role.
- **Idempotent writes**: Always use `UPSERT`/`MERGE` (not `CREATE`/`INSERT`) to avoid collisions when Soft/Hard paths race.
- **Versioning/locking**: If the Indexer writes, it should lock or make the record immutable (e.g., `is_on_chain = true`) to prevent reverting confirmed on-chain state.
- **Ghost writes**: Pending tx that never lands must expire. Use TTL/cron to mark `pending_tx` stale after a threshold (e.g., 1 hour).

## Performance Gotchas (Immediate)
- **Graph permission trap**: Avoid dynamic graph traversals in `PERMISSIONS`. Store permission data directly on the record.
- **Over-indexing**: Each index adds a write tax. Only index fields you query in `WHERE`.
- **Huge transactions**: Don’t wrap thousands of events in one transaction. Commit per block or per ~100 events.

## Practical Focus
- Keep Indexer batch sizes small to avoid blocking interactive users.
- You don’t need major architectural changes until you’re sustaining ~1,000+ writes/sec.
