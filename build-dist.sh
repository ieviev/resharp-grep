#!/usr/bin/env bash
set -euo pipefail

TARGETS=(
    x86_64-unknown-linux-gnu
    aarch64-unknown-linux-musl
    aarch64-apple-darwin
    x86_64-pc-windows-gnu
)

NAMES=(
    re-x86_64-linux
    re-aarch64-linux
    re-aarch64-macos
    re-x86_64-windows.exe
)

NEEDS_BUILD_STD=(
    x86_64-pc-windows-gnu
)

mkdir -p dist

for i in "${!TARGETS[@]}"; do
    target="${TARGETS[$i]}"
    name="${NAMES[$i]}"
    echo "building $target..."

    flags=(--release --target "$target")
    for t in "${NEEDS_BUILD_STD[@]}"; do
        if [[ "$target" == "$t" ]]; then
            flags+=(-Zbuild-std)
            break
        fi
    done

    cargo zigbuild "${flags[@]}"

    src="target/$target/release/re"
    [[ "$target" == *windows* ]] && src+=".exe"
    cp "$src" "dist/$name"
done

echo "done:"
ls -lh dist/
