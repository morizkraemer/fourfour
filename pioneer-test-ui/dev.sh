#!/bin/bash
# Fast dev loop: rebuild app bundle, kill old instance, relaunch.
# Frontend is embedded in the binary so we need a full build each time.
set -e
cd "$(dirname "$0")/.."

APP="target/debug/bundle/macos/Pioneer Test UI.app"

# Kill existing instance
pkill -f "Pioneer Test UI" 2>/dev/null || true
sleep 0.3

# Build the bundle
cargo tauri build --debug --bundles app 2>&1 | grep -v "^warning:"

open "$APP"
