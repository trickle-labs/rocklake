#!/usr/bin/env bash
set -euo pipefail

binary=${1:?usage: test_release_artifact.sh BINARY}
catalog_dir=$(mktemp -d)
trap 'rm -rf "$catalog_dir"' EXIT

"$binary" --version
"$binary" --version --output json | grep -q '"version"'
"$binary" doctor --catalog "$catalog_dir/catalog" --output json | grep -q '"ready": true'
