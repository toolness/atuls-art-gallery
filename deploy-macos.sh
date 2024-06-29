#!/usr/bin/env sh

set -e

sh build-initial-db.sh

cd rust
rm -f target/debug/libplugin.dylib
cargo build -p plugin --target=aarch64-apple-darwin
# This might require running `rustup target add x86_64-apple-darwin`
cargo build -p plugin --target=x86_64-apple-darwin
lipo -create target/aarch64-apple-darwin/debug/libplugin.dylib target/x86_64-apple-darwin/debug/libplugin.dylib -output target/debug/libplugin.dylib
cd ..

rm -rf dist/files
mkdir dist/files
godot --headless --export-debug "macOS"
butler push dist/files/gallery.zip toolness/gallery:macos-universal
