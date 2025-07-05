#!/bin/bash
set -e

# Ah! The joy of cross-compiling pure-rust binaries
# Add the corresponding rust target then use [https://github.com/rust-cross/cargo-zigbuild](Zig) to simplify cross-compilation *significantly*

export ZIG_GLOBAL_CACHE_DIR=$PWD/target/zig-cache # [src](https://github.com/ziglang/zig/issues/19400) - global cache in home by default for Zig

# Linux
cargo zigbuild --release --target x86_64-unknown-linux-gnu # x86
cargo zigbuild --release --target aarch64-unknown-linux-gnu # arm64

# Windows
cargo zigbuild --release --target x86_64-pc-windows-gnu # x86
cargo zigbuild --release --target aarch64-pc-windows-gnullvm # arm64

# MacOS
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin