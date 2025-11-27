# Database Backup Scripts

This directory contains scripts for backing up, restoring, and managing your PostgreSQL/TimescaleDB database.

## Prerequisites

- Docker and docker-compose installed
- Database container running (name: `database`)
- Write permissions to create `./backups` directory

## Scripts Overview

### 1. `backup-db.sh` - Create Database Backup

Creates a compressed backup of the entire database including schema, data, triggers, and functions.

**Usage:**
```bash
./scripts/backup-db.sh
```

**Features:**
- Creates compressed `.dump` file using PostgreSQL custom format
- Automatically cleans up old backups (keeps last 7)
- Generates SHA256 checksum for integrity verification
- Shows backup size and location

**Output:**
- Backup file: `./backups/storage_backup_YYYYMMDD_HHMMSS.dump`
- Checksum file: `./backups/storage_backup_YYYYMMDD_HHMMSS.dump.sha256`

---

### 2. `restore-db.sh` - Restore Database from Backup

Restores the database from a backup file.

**Usage:**
```bash
# Restore from specific backup
./scripts/restore-db.sh ./backups/storage_backup_20241118_120000.dump

# Restore from latest backup
./scripts/restore-db.sh latest
```

**Features:**
- Verifies backup integrity using checksum (if available)
- Creates safety backup before restore
- Cleans and recreates all database objects
- Provides detailed restore progress
- Shows database statistics after restore

**⚠️ Warning:** This will DELETE all existing data in the database!

---

### 3. `verify-db-triggers.sh` - Verify Database Integrity

Checks that all expected triggers, functions, and tables are present after a restore.

**Usage:**
```bash
./scripts/verify-db-triggers.sh
```

**What it checks:**
- ✓ All 26 expected triggers
- ✓ Database functions (~25+ expected)
- ✓ Key tables (account, atom, triple, position, vault, etc.)
- ✓ Indexes
- ✓ Extensions (TimescaleDB, pgai, etc.)
- ✓ Stats table initialization

**Exit codes:**
- `0` - All checks passed
- `1` - Issues detected (review output)

---

### 4. `list-backups.sh` - List Available Backups

Displays all available backups with details.

**Usage:**
```bash
./scripts/list-backups.sh
```

**Shows:**
- Backup filename
- File size
- Creation date
- Checksum availability
- Total space used

---

### 5. `cleanup-old-backups.sh` - Remove Old Backups

Removes old backups based on retention policy.

**Usage:**
```bash
# Keep last 7 backups (default)
./scripts/cleanup-old-backups.sh

# Keep last 14 backups
./scripts/cleanup-old-backups.sh 14

# Remove all backups
./scripts/cleanup-old-backups.sh all
```

---

## Complete Workflow Examples

### Regular Backup Workflow

```bash
# 1. Create a backup
./scripts/backup-db.sh

# 2. List all backups
./scripts/list-backups.sh

# 3. Clean up old backups (keep last 7)
./scripts/cleanup-old-backups.sh
```

### Restore and Verify Workflow

```bash
# 1. Restore from latest backup
./scripts/restore-db.sh latest

# 2. Verify database integrity
./scripts/verify-db-triggers.sh

# 3. Test your application
# ... run your tests here ...
```

### Disaster Recovery

```bash
# If something goes wrong, restore from safety backup
./scripts/restore-db.sh ./backups/pre_restore_safety_YYYYMMDD_HHMMSS.dump
```

---

## Backup Storage Structure

```
./backups/
├── storage_backup_20241118_120000.dump       # Main backup
├── storage_backup_20241118_120000.dump.sha256 # Checksum
├── storage_backup_20241117_120000.dump
├── storage_backup_20241117_120000.dump.sha256
└── pre_restore_safety_20241118_130000.dump   # Auto safety backup
```

---

## Automation

### Local Development (cron)

Add to your crontab for daily backups:

```bash
# Daily backup at 2 AM
0 2 * * * cd /path/to/intuition-rs && ./scripts/backup-db.sh >> ./backups/backup.log 2>&1
```

### GKE Production (CronJob)

See the main planning document for Kubernetes CronJob configuration for weekly snapshots in GKE.

---

## Important Notes

### What's Included in Backups

✅ **Included:**
- All tables and data
- All triggers (26+ triggers)
- All functions (~25+ functions)
- Indexes
- Constraints
- Views and materialized views
- Extensions configuration
- Sequences

❌ **Not Included:**
- Docker volumes
- Application logs
- Redis data
- IPFS data
- Configuration files

### Trigger Safety

The backup process is **trigger-safe** because:

1. `pg_dump` exports the **definition** of triggers (not their execution)
2. Triggers are stored as metadata in PostgreSQL
3. On restore, triggers are recreated exactly as they were
4. Data is restored in a trigger-safe order

### Performance Considerations

- **Backup time**: Depends on database size (~1-10 minutes typical)
- **Restore time**: Usually slower than backup (~2-20 minutes)
- **Compression**: Level 9 (best compression, slower speed)
- **Format**: Custom format (optimized for pg_restore)

### Troubleshooting

**Issue: "Container 'database' is not running"**
```bash
# Start the database
docker-compose -f docker/docker-compose-shared.yml up -d database
```

**Issue: "Permission denied"**
```bash
# Make scripts executable
chmod +x scripts/*.sh
```

**Issue: "Backup file corrupted"**
```bash
# Check the checksum
shasum -a 256 -c ./backups/storage_backup_YYYYMMDD_HHMMSS.dump.sha256
```

**Issue: "Not enough disk space"**
```bash
# Clean up old backups
./scripts/cleanup-old-backups.sh 3  # Keep only 3 latest
```

---

## Next Steps

1. **Test locally**: Create a backup and restore it to verify everything works
2. **Set up automation**: Add cron job for regular backups
3. **Plan for GKE**: Implement CronJob for weekly snapshots with GCS storage
4. **Document retention**: Define backup retention policy (daily/weekly/monthly)
5. **Test disaster recovery**: Practice full restore procedure

---

## Support

For issues or questions:
- Check logs in `/tmp/backup.log` and `/tmp/restore.log`
- Verify container status: `docker ps`
- Check database logs: `docker logs database`
