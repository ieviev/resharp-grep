#!/usr/bin/env bash
set -euo pipefail

version=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
tag="v$version"

binaries=(dist/*)
if [[ ${#binaries[@]} -eq 0 ]]; then
    echo "no binaries in dist/, run build-dist.sh first"
    exit 1
fi

echo "creating release $tag with:"
ls -lh dist/

gh release create "$tag" "${binaries[@]}" \
    --title "$tag" \
    --generate-notes
