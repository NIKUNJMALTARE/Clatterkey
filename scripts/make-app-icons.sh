#!/usr/bin/env bash
# Regenerates the app icon in src-tauri/icons/: `npm run make-app-icons`.
#
# The generator (src-tauri/examples/make_app_icons) draws the icon and writes
# the PNG files, icon.ico and an iconset. The .icns is then packed from that
# iconset by iconutil, which is part of macOS, so it is rebuilt only on a
# Mac. All of it is committed; the app, the tests and CI never run this.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo run --quiet --manifest-path src-tauri/Cargo.toml --example make-app-icons

if command -v iconutil >/dev/null 2>&1; then
	iconutil --convert icns --output src-tauri/icons/icon.icns src-tauri/target/app-icons/icon.iconset
	echo "packed src-tauri/icons/icon.icns"
else
	echo "iconutil not found (it is part of macOS): src-tauri/icons/icon.icns was not rebuilt" >&2
fi
