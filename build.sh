#!/bin/bash
set -e

# Ah! The joy of cross-compiling pure-rust binaries
# Add the corresponding rust target then use [https://github.com/rust-cross/cargo-zigbuild](Zig) to simplify cross-compilation *significantly*

# I should probably learn GitHub workflows...

# Compilation
export ZIG_GLOBAL_CACHE_DIR=$PWD/target/zig-cache # [src](https://github.com/ziglang/zig/issues/19400) - global cache in home by default for Zig

## Linux
cargo zigbuild --release --target x86_64-unknown-linux-gnu # x86
cargo zigbuild --release --target aarch64-unknown-linux-gnu # arm64

## Windows
cargo zigbuild --release --target x86_64-pc-windows-gnu # x86
cargo zigbuild --release --target aarch64-pc-windows-gnullvm # arm64

## MacOS
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Move binaries into dist directory
OUTPUT_DIR="./target/dist"
BINARY_NAME="frhop"
TARGETS=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "aarch64-pc-windows-gnullvm"
    "x86_64-pc-windows-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-unknown-linux-gnu"
)

mkdir -p "$OUTPUT_DIR"

for TARGET in "${TARGETS[@]}"; do
    BIN_PATH="target/$TARGET/release/$BINARY_NAME"
    OUT_NAME="${BINARY_NAME}-${TARGET}"
    
    # add .exe for winodows
    if [[ "$TARGET" == *windows* ]]; then
        BIN_PATH="${BIN_PATH}.exe"
    fi

    if [[ -f "$BIN_PATH" ]]; then
        cp "$BIN_PATH" "$OUTPUT_DIR/$OUT_NAME"
        echo "Collected $BIN_PATH -> $OUTPUT_DIR/$OUT_NAME"
    else
        echo "$BIN_PATH not found, did you build for $TARGET?" >&2
    fi
done

echo "Done!"
