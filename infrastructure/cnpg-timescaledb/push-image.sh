#!/bin/bash
set -e

# Configuration
IMAGE_NAME="cnpg-timescaledb"
IMAGE_TAG="17-timescale-2.17"
REGISTRY="ghcr.io/0xintuition"
FULL_IMAGE="${REGISTRY}/${IMAGE_NAME}:${IMAGE_TAG}"

echo "📦 Pushing CloudNativePG + TimescaleDB image to registry..."
echo "   Image: ${FULL_IMAGE}"
echo ""

# Check if logged in to ghcr.io
if ! docker info | grep -q "ghcr.io"; then
    echo "⚠️  Not logged in to ghcr.io"
    echo "   Please run: echo \$GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin"
    exit 1
fi

# Push versioned tag
echo "Pushing ${FULL_IMAGE}..."
docker push ${FULL_IMAGE}

# Push latest tag
echo "Pushing ${REGISTRY}/${IMAGE_NAME}:latest..."
docker push ${REGISTRY}/${IMAGE_NAME}:latest

echo ""
echo "✅ Push complete!"
echo ""
echo "🎯 Next steps:"
echo "   1. Update your GKE cluster manifest to use: ${FULL_IMAGE}"
echo "   2. Apply the updated manifest: kubectl apply -f your-cluster-manifest.yaml"
echo ""
echo "📝 Example cluster configuration:"
echo "   spec:"
echo "     imageName: ${FULL_IMAGE}"
