#!/usr/bin/env bash
# Assemble dist/tuios-rs-linux-x86_64.tar.gz (+ .sha256) from a release build:
# the binary, LICENSE, install notes and every dependency's license files.
set -euo pipefail
name=tuios-rs-linux-x86_64
out="dist/$name"
rm -rf dist && mkdir -p "$out/licenses"
cp target/release/tuios-rs "$out/"
cp LICENSE "$out/LICENSE"
cp docs/INSTALL-ARCH.md "$out/README.md"
cargo metadata --format-version 1 \
  | jq -r '.packages[] | select(.source != null) | "\(.name)-\(.version)\t\(.manifest_path)"' \
  | while IFS=$'\t' read -r id manifest; do
      dir=$(dirname "$manifest")
      for f in "$dir"/LICENSE* "$dir"/COPYING* "$dir"/NOTICE* "$dir"/UNLICENSE*; do
        [ -f "$f" ] || continue
        mkdir -p "$out/licenses/$id"
        cp "$f" "$out/licenses/$id/"
      done
    done
tar -C dist -czf "dist/$name.tar.gz" "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
ls -l dist
