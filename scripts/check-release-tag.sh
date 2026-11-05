#!/bin/sh
# Usage: check-release-tag.sh vX.Y.Z
# Fails unless the tag, minus its leading "v", equals the workspace version.
set -eu
tag="$1"
# Read the manifests, not Cargo.lock (cargo pkgid would report a stale lock's version).
version=$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "crablink-server") | .version')
if [ "$tag" != "v$version" ]; then
  echo "tag $tag does not match workspace version $version" >&2
  exit 1
fi
echo "tag $tag matches workspace version $version"
