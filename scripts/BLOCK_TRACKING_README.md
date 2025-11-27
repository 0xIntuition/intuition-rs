# Block Tracking in Backup/Restore Scripts

## Overview

The backup and restore scripts now automatically track the latest block number ingested in the database. This is critical for blockchain indexing systems to know where to resume indexing after a database restore.

## Features Added

### 1. Automatic Block Capture During Backup

When you run `./scripts/backup-db.sh`, it now:
- Queries all relevant tables (`atom`, `deposit`, `redemption`, `share_price_change`, `triple`, `signal`, `event`, `vault`, `position`)
- Finds the maximum `block_number` across all tables
- Saves this information to a `.block_info` file alongside the backup

**Example Output:**
```
[INFO] Capturing latest block information...
[INFO] Latest block ingested: 1317
[INFO] Block metadata saved to: ./backups/storage_backup_20251118_111753.dump.block_info
```

**Block Info File Contents:**
```
Backup File: storage_backup_20251118_111753.dump
Backup Timestamp: 2025-11-18 14:17:53 UTC
Latest Block Ingested: 1317
Stats Table Block: 1317
Database Name: storage
```

### 2. Block Display During Restore

When you run `./scripts/restore-db.sh`, it now:
- Shows the block information from the backup being restored
- Verifies the block number after restore completes
- Provides clear guidance on which block to resume indexing from

**Example Output:**
```
[INFO] Backup block information:
  Latest Block Ingested: 1317
  Stats Table Block: 1317

... (restore process) ...

[INFO] Checking latest block after restore...
[INFO]   - Latest block restored: 1317
[WARN] ⚠️  Resume indexing from block: 1317

[INFO] Next steps:
  1. Run './scripts/verify-db-triggers.sh' to verify all triggers
  2. Run './scripts/check-latest-block.sh' for detailed block analysis
  3. Resume indexer from block 1317
  4. Test your application to ensure everything works correctly
```

### 3. Standalone Block Checker

New script: `./scripts/check-latest-block.sh`

This script provides detailed analysis of block numbers across all tables.

**Usage:**
```bash
# Basic check
./scripts/check-latest-block.sh

# JSON output (for automation)
./scripts/check-latest-block.sh --json

# Save to file
./scripts/check-latest-block.sh --save
```

**Example Output:**
```
==================== LATEST BLOCK ANALYSIS ====================
[INFO] Stats table reports:
  Block: 1317
  Timestamp: 2025-11-18 14:06:37+00

==================== BLOCK NUMBERS BY TABLE ====================
      table_name     | max_block_number | row_count |   note
--------------------+------------------+-----------+----------
 position           |             1317 |       182 | ← LATEST
 deposit            |             1317 |       187 | ← LATEST
 event              |             1317 |       364 | ← LATEST
 vault              |             1317 |       238 | ← LATEST
 atom               |             1317 |       109 | ← LATEST
 share_price_change |             1317 |       256 | ← LATEST
 signal             |             1317 |       192 | ← LATEST
 redemption         |             1311 |         5 |
 triple             |             1274 |        65 |
 fee_transfer       |                  |         0 | (empty)

==================== SUMMARY ====================

📊 Latest Block Information:

  ╔════════════════════════════════════════╗
  ║  Latest Block Ingested: 1317        ║
  ║  Stats Table Block:     1317        ║
  ╚════════════════════════════════════════╝

[INFO] ✓ Stats table is in sync with actual data
[INFO] Indexer should resume from block: 1317
```

### 4. Enhanced Backup Listing

The `./scripts/list-backups.sh` script now shows block information for each backup.

**Example Output:**
```
==================== AVAILABLE BACKUPS ====================
[INFO] Found 2 backup(s) in ./backups

FILENAME                                 SIZE       DATE                 BLOCK      CHECKSUM
---------------------------------------- ---------- -------------------- ---------- ----------
storage_backup_20251118_111753.dump      796K       Nov 18 11:17         1317       ✓
storage_backup_20251118_110757.dump      784K       Nov 18 11:07         -          ✓

[INFO] Note: BLOCK column shows the latest block ingested in each backup
```

## Tables Monitored for Block Numbers

The following tables are queried to determine the latest block:

| Table | Column | Type |
|-------|--------|------|
| `atom` | `block_number` | numeric |
| `deposit` | `block_number` | numeric |
| `redemption` | `block_number` | numeric |
| `share_price_change` | `block_number` | numeric |
| `triple` | `block_number` | numeric |
| `signal` | `block_number` | numeric |
| `event` | `block_number` | numeric |
| `fee_transfer` | `block_number` | numeric |
| `vault` | `block_number` | bigint |
| `position` | `block_number` | bigint |
| `stats` | `last_processed_block_number` | numeric |

## Use Cases

### 1. Regular Backups with Block Tracking

```bash
# Create a backup (automatically captures block info)
./scripts/backup-db.sh

# List backups with block numbers
./scripts/list-backups.sh
```

### 2. Disaster Recovery

```bash
# Restore from backup
./scripts/restore-db.sh latest

# The script will show:
# - Block number from the backup metadata
# - Block number after restore verification
# - Clear instruction to resume indexing from that block

# Verify the restored block
./scripts/check-latest-block.sh
```

### 3. Automation & Monitoring

```bash
# Get block info as JSON for scripts/monitoring
./scripts/check-latest-block.sh --json

# Example output:
# {
#   "latest_block": 1317,
#   "stats_block": 1317,
#   "stats_timestamp": "2025-11-18 14:06:37+00",
#   "in_sync": true
# }

# Use in automation:
BLOCK=$(./scripts/check-latest-block.sh --json | jq -r '.latest_block')
echo "Resume indexing from block: $BLOCK"
```

### 4. Before/After Comparison

```bash
# Before a restore, check current block
./scripts/check-latest-block.sh --save

# After restore, check again
./scripts/check-latest-block.sh

# Compare to ensure data integrity
```

## File Structure

After running a backup, you'll have these files:

```
./backups/
├── storage_backup_20251118_111753.dump           # Main backup file
├── storage_backup_20251118_111753.dump.sha256    # Checksum for integrity
└── storage_backup_20251118_111753.dump.block_info # Block metadata
```

## Integration with GKE

When setting up automated backups in GKE, the block metadata can be:
1. Uploaded to Google Cloud Storage alongside the backup
2. Used in monitoring/alerting systems
3. Stored in a metadata database for tracking backup history
4. Used to automatically configure indexer restart points

**Example GKE CronJob addition:**
```yaml
# After backup completes, upload block metadata
- gsutil cp /backups/*.block_info gs://your-bucket/backups/metadata/
```

## Troubleshooting

### Block Number Mismatch

If the max block across tables doesn't match the stats table:

```bash
./scripts/check-latest-block.sh
# Will show: ⚠ Stats table block (1317) differs from max block (1320)
```

This can happen if:
- Stats table update triggers failed
- Manual data insertion bypassed triggers
- Database restore was interrupted

**Resolution:** The maximum block across all tables is the authoritative value.

### Missing Block Info File

Old backups created before this feature won't have `.block_info` files. They'll show `-` in the BLOCK column:

```
storage_backup_20251118_110757.dump      784K       Nov 18 11:07         -          ✓
```

You can still restore these backups, but you'll need to manually check the block after restore:

```bash
./scripts/restore-db.sh ./backups/old_backup.dump
./scripts/check-latest-block.sh
```

## Summary

✅ **Automatic block tracking** in every backup
✅ **Block verification** during restore
✅ **Detailed analysis** with check-latest-block.sh
✅ **JSON output** for automation
✅ **Enhanced backup listing** showing block numbers
✅ **Clear guidance** on where to resume indexing

This ensures you always know exactly where to resume your blockchain indexer after a database restore, preventing data gaps or duplicate processing.
