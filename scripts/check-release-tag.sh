#!/bin/sh
# Usage: check-release-tag.sh vX.Y.Z
# Fails unless every workspace package shares one version and the tag, minus its
# leading "v", equals it. When $GITHUB_OUTPUT is set, writes `version` and
# `prerelease` (true when the version has a "-" suffix, e.g. 1.0.0-rc.1) to it.
set -eu
tag="$1"
# Read the manifests, not Cargo.lock (cargo pkgid would report a stale lock's version).
versions=$(cargo metadata --no-deps --format-version 1 | jq -r '[.packages[].version] | unique | .[]')
if [ "$(printf '%s\n' "$versions" | wc -l)" -ne 1 ]; then
  echo "workspace packages disagree on version: $(printf '%s' "$versions" | tr '\n' ' ')" >&2
  exit 1
fi
version="$versions"
if [ "$tag" != "v$version" ]; then
  echo "tag $tag does not match workspace version $version" >&2
  exit 1
fi
echo "tag $tag matches workspace version $version"
case "$version" in
  *-*) prerelease=true ;;
  *) prerelease=false ;;
esac
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  printf 'version=%s\nprerelease=%s\n' "$version" "$prerelease" >>"$GITHUB_OUTPUT"
fi
