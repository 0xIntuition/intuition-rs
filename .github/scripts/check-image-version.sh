#!/usr/bin/env bash
# Validates the inputs of an image build dispatch.
#
# Usage: check-image-version.sh <Cargo.toml> <version> <also_latest> <git ref>
#
# Fails unless <version> is plain semver (X.Y.Z), equals the [package] version
# of <Cargo.toml>, and :latest is only requested from refs/heads/v2_0.
set -euo pipefail

manifest=$1
version=$2
also_latest=$3
ref=$4

if ! [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::version '$version' is not X.Y.Z"
  exit 1
fi

# The version key inside [package] only; a workspace-inherited version
# (version.workspace = true) yields nothing and fails below.
crate=$(sed -n '/^\[package\]/,/^\[/{s/^version *= *"\([^"]*\)".*/\1/p;}' "$manifest" | head -n1)
if [ "$crate" != "$version" ]; then
  echo "::error::version '$version' does not match $manifest ('$crate')"
  exit 1
fi

if [ "$also_latest" = "true" ] && [ "$ref" != "refs/heads/v2_0" ]; then
  echo "::error::also_latest is only allowed from refs/heads/v2_0 (got $ref)"
  exit 1
fi

echo "version $version ok"
