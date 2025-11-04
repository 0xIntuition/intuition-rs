# CloudNativePG + TimescaleDB Migration Issue - Root Cause & Fix

## Problem Summary

Hasura migrations are failing in GKE with the error:
```
extension "timescaledb" has no installation script nor update path for version "2.22.1"
```

## Root Cause Analysis

The image `ghcr.io/clevyr/cloudnativepg-timescale:17-ts2` has **two critical bugs**:

### Issue 1: Broken TimescaleDB Extension Files
- **Advertised version**: 2.22.1 (in `pg_available_extensions`)
- **Available install scripts**: Only up to 2.19.3
- **Missing file**: `/usr/share/postgresql/17/extension/timescaledb--2.22.1.sql`
- **Result**: `CREATE EXTENSION timescaledb` fails

### Issue 2: Missing Preload Configuration
- **Required**: `shared_preload_libraries = 'timescaledb'`
- **Actual**: Empty string
- **Result**: Even if extension installs, it cannot be loaded

## Solutions for GKE

### Option 1: Use PostgreSQL with pgvector (Recommended for production)
If you don't strictly need TimescaleDB's time-series features, use standard CNPG images:

```yaml
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: your-cluster
spec:
  instances: 3
  imageName: ghcr.io/cloudnative-pg/postgresql:17

  postgresql:
    parameters:
      max_connections: "200"
      shared_buffers: "256MB"
```

Then install TimescaleDB extension manually after cluster creation if needed.

### Option 2: Build Your Own CNPG + TimescaleDB Image

Create a Dockerfile:

```dockerfile
FROM ghcr.io/cloudnative-pg/postgresql:17

USER root

# Install TimescaleDB
RUN apt-get update && \\
    apt-get install -y postgresql-17-timescaledb-2 && \\
    apt-get clean && \\
    rm -rf /var/lib/apt/lists/*

# Configure shared_preload_libraries
RUN echo "shared_preload_libraries = 'timescaledb'" >> /usr/share/postgresql/postgresql.conf.sample

USER 26

```

Build and push:
```bash
docker build -t your-registry/cnpg-timescaledb:17-2.17.2 .
docker push your-registry/cnpg-timescaledb:17-2.17.2
```

### Option 3: Fix Migrations to Specify Version

Modify your migration file to explicitly use version 2.19.3:

```sql
-- Instead of:
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Use:
CREATE EXTENSION IF NOT EXISTS timescaledb VERSION '2.19.3';
```

**Location**: `/Users/leboiko/Documents/temp/temp2/intuition-rs/infrastructure/hasura/migrations/intuition/1729947331633_consolidated_basic_structure/up.sql`

Change line that creates the extension from:
```sql
CREATE EXTENSION IF NOT EXISTS timescaledb WITH SCHEMA public;
```

To:
```sql
CREATE EXTENSION IF NOT EXISTS timescaledb VERSION '2.19.3' WITH SCHEMA public;
```

### Option 4: Use a Different TimescaleDB Image (Not Tested)

Try other CNPG-compatible images:
- Check Timescale's official registry for CNPG images
- Search for community-built CNPG + TimescaleDB images

## Immediate Fix for GKE

**Quick Solution**: Option 3 (Modify migrations)

1. Update your migration file to use version 2.19.3 explicitly
2. Rebuild your hasura-migrations image
3. Push to `ghcr.io/0xintuition/hasura-migrations:latest`
4. Redeploy to GKE

## Testing Locally

Your local kind cluster setup successfully reproduced the issue:
- ✅ CloudNativePG operator installed
- ✅ TimescaleDB cluster deployed
- ✅ Hasura GraphQL Engine connected
- ✅ Migration error reproduced
- ✅ Root cause identified

## Files Modified

1. `/Users/leboiko/Documents/temp/temp2/intuition-rs/cnpg-cluster.yaml` - Local test cluster
2. `/Users/leboiko/Documents/temp/temp2/intuition-rs/hasura-deployment.yaml` - Local Hasura deployment

## Next Steps

1. Choose a solution (Option 3 is fastest)
2. Test locally in your kind cluster
3. Apply to GKE
4. Report the bug to clevyr: https://github.com/clevyr/docker-timescale/issues

## Kubernetes Context

Your current kubectl contexts:
- Local testing: `kind-cnpg-local` (created for this debugging)
- Production: `gke_be-cluster_us-west2_debug-cluster` (your GKE cluster)

To switch back to GKE:
```bash
kubectl config use-context gke_be-cluster_us-west2_debug-cluster
```

To delete local cluster when done:
```bash
kind delete cluster --name cnpg-local
```
