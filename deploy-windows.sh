#!/usr/bin/env sh

set -e

sh build-plugin.sh
sh build-initial-db.sh
rm -rf dist/files
mkdir dist/files

# For some bizarre reason the command-line version of godot,
# which will actually exit once the exporting is complete, is
# called `godot_console` and can't be renamed. On my system I have
# a godot.bat wrapper that wraps it so I don't have to constantly
# type `godot_console` from the command-line. This means I need to put
# godot in a separate directory that's not on my PATH so that
# `godot.exe` doesn't shadow `godot.bat`.  And `godot_console` has
# to be in the same directory as `godot.exe`.
#
# This means I now need to have a $GODOT_PATH environment variable that
# points to the actual godot directory so I can run the console version
# in this script. Awesome.
$GODOT_PATH/godot_console --headless --export-debug "Windows Desktop"

butler push dist/files toolness/gallery:windows
