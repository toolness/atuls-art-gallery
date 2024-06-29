#!/usr/bin/env sh

set -e

sh build-plugin.sh
sh build-initial-db.sh
rm -rf dist/files
mkdir dist/files
godot_console --headless --export-debug "Windows Desktop"
butler push dist/files toolness/gallery:windows
