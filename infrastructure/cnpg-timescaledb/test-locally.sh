#!/bin/bash
set -e

# Configuration
IMAGE_NAME="cnpg-timescaledb"
IMAGE_TAG="17-timescale-2.17"
REGISTRY="ghcr.io/0xintuition"
FULL_IMAGE="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}"

echo "🧪 Testing CloudNativePG + TimescaleDB image in local kind cluster..."
echo "   Context: kind-cnpg-local"
echo "   Image: ${FULL_IMAGE}"
echo ""

# Switch to kind context
kubectl config use-context kind-cnpg-local

# Load image into kind cluster
echo "📥 Loading image into kind cluster..."
kind load docker-image ${FULL_IMAGE} --name cnpg-local

# Update the cluster manifest
echo "📝 Updating cluster manifest..."
cat > /tmp/test-cnpg-cluster.yaml <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: test-timescale-cluster
spec:
  instances: 1
  imageName: ${FULL_IMAGE}

  enableSuperuserAccess: true
  superuserSecret:
    name: test-postgres-superuser

  postgresql:
    parameters:
      max_connections: "200"
      shared_buffers: "256MB"

  bootstrap:
    initdb:
      database: storage
      owner: postgres

  storage:
    size: 1Gi
    storageClass: standard
---
apiVersion: v1
kind: Secret
metadata:
  name: test-postgres-superuser
type: Opaque
stringData:
  username: postgres
  password: postgres
EOF

# Clean up any existing test cluster
echo "🧹 Cleaning up existing test cluster..."
kubectl delete cluster test-timescale-cluster 2>/dev/null || true
kubectl delete secret test-postgres-superuser 2>/dev/null || true
sleep 5

# Apply the test cluster
echo "🚀 Creating test cluster..."
kubectl apply -f /tmp/test-cnpg-cluster.yaml

# Wait for cluster to be ready
echo "⏳ Waiting for cluster to become ready (up to 60s)..."
kubectl wait --for=condition=Ready --timeout=60s pod -l cnpg.io/cluster=test-timescale-cluster || {
    echo "❌ Cluster failed to become ready. Checking logs..."
    kubectl logs -l cnpg.io/cluster=test-timescale-cluster --tail=50
    exit 1
}

echo ""
echo "✅ Cluster is ready! Testing TimescaleDB..."

# Test TimescaleDB installation
echo "Testing TimescaleDB extension..."
kubectl exec test-timescale-cluster-1 -- psql -U postgres -d storage -c "CREATE EXTENSION IF NOT EXISTS timescaledb VERSION '2.19.3';" || {
    echo "❌ Failed to create TimescaleDB extension"
    kubectl logs test-timescale-cluster-1
    exit 1
}

# Verify extension is loaded
kubectl exec test-timescale-cluster-1 -- psql -U postgres -d storage -c "SELECT extname, extversion FROM pg_extension WHERE extname = 'timescaledb';"

# Test hypertable creation
echo ""
echo "Testing hypertable creation..."
kubectl exec test-timescale-cluster-1 -- psql -U postgres -d storage <<'EOSQL'
CREATE TABLE test_hypertable (
  time TIMESTAMPTZ NOT NULL,
  value DOUBLE PRECISION
);

SELECT create_hypertable('test_hypertable', 'time');

INSERT INTO test_hypertable VALUES (NOW(), 1.0);

SELECT * FROM test_hypertable;
EOSQL

echo ""
echo "✅ All tests passed!"
echo ""
echo "🎉 The custom image works correctly!"
echo ""
echo "📝 Next steps:"
echo "   1. Push image to registry: ./push-image.sh"
echo "   2. Update your GKE cluster to use: ${FULL_IMAGE}"
echo ""
echo "🧹 To clean up test cluster:"
echo "   kubectl delete cluster test-timescale-cluster"
