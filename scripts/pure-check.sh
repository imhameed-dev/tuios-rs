#!/usr/bin/env bash
# Offline check of the modules that use only std: copies them into a scratch
# crate with no dependencies and runs fmt, clippy and their unit tests.
# Usage: scripts/pure-check.sh <scratch-dir>
set -euo pipefail
src="$(cd "$(dirname "$0")/.." && pwd)/src"
dir="$1"
rm -rf "$dir" && mkdir -p "$dir/src"
cat > "$dir/Cargo.toml" <<'T'
[package]
name = "pure"
version = "0.0.0"
edition = "2021"
rust-version = "1.87"
[dependencies]
T
cp -r "$src/layout" "$dir/src/layout"
cp "$src"/{input,modal,render,workspace}.rs "$dir/src/"
printf 'pub mod input;\npub mod layout;\npub mod modal;\npub mod render;\npub mod workspace;\n' > "$dir/src/lib.rs"
cd "$dir"
cargo clippy --all-targets -- -D warnings
cargo test 2>&1 | grep -E '^test result|FAILED|panicked'
