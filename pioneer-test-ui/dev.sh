#!/bin/bash
# Build debug .app bundle and launch it
set -e
cd "$(dirname "$0")/.."
cargo tauri build --debug --bundles app 2>&1 | grep -v "^warning:"
open "target/debug/bundle/macos/Pioneer Test UI.app"
