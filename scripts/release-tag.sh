#!/usr/bin/env bash
# Tag the current master HEAD with the version in Cargo.toml and push it —
# release.yml triggers on "v*" tag push (cross-platform build, cosign sign,
# SLSA provenance, GitHub Release). Run this after a release-bump PR merges.
set -euo pipefail

cd "$(dirname "$0")/.."

error() {
  echo "$@" >&2
  exit 1
}

current_branch=$(git rev-parse --abbrev-ref HEAD)
[[ "$current_branch" == "master" ]] || error "run this from master (on: $current_branch)"
git diff --quiet && git diff --cached --quiet || error "working tree not clean"

git fetch --quiet origin master
git checkout --quiet master
git pull --ff-only --quiet origin master

version=$(grep -m1 '^version = "' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
[[ -n "$version" ]] || error "could not read version from Cargo.toml"
tag="v$version"

git rev-parse -q --verify "refs/tags/$tag" >/dev/null && error "tag $tag already exists"

git tag -a "$tag" -m "$tag"
git push origin "$tag"

echo "Pushed $tag — release.yml: https://github.com/getappz/agentflare/actions/workflows/release.yml"
