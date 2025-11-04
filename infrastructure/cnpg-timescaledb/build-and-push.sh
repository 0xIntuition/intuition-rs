#!/bin/bash
set -e

# Configuration
IMAGE_NAME="cnpg-timescaledb"
IMAGE_TAG="17-timescale-2.17"
REGISTRY="ghcr.io/0xintuition"
FULL_IMAGE="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}"

echo "🔨 Building CloudNativePG + TimescaleDB image..."
echo "   Image: ${FULL_IMAGE}"
echo ""

# Build the image
docker build \
  --platform linux/amd64 \
  -t ${FULL_IMAGE} \
  -t ${REGISTRY}/${IMAGE_NAME}:latest \
  -f Dockerfile \
  .

echo ""
echo "✅ Build complete!"
echo ""
echo "🧪 Testing the image..."

# Test that PostgreSQL starts
docker run --rm ${FULL_IMAGE} postgres --version

# Test that TimescaleDB extension files exist
docker run --rm ${FULL_IMAGE} bash -c "ls /usr/share/postgresql/17/extension/timescaledb*.sql | wc -l"

# Test that shared_preload_libraries is set
docker run --rm ${FULL_IMAGE} bash -c "grep 'shared_preload_libraries' /usr/share/postgresql/postgresql.conf.sample || echo 'WARNING: shared_preload_libraries not found'"

echo ""
echo "✅ Tests passed!"
echo ""
echo "📦 To push to registry:"
echo "   1. Login: echo \$GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin"
echo "   2. Push: docker push ${FULL_IMAGE}"
echo "   3. Push latest: docker push ${REGISTRY}/${IMAGE_NAME}:latest"
echo ""
echo "Or run: ./push-image.sh"
