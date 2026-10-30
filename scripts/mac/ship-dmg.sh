#!/bin/bash
set -euo pipefail

HOST="${MAC_HOST:-mbp}"
REMOTE_DIR="${REMOTE_DIR:-dev/build/fourfour-dmg}"
BUNDLE_REMOTE="/tmp/fourfour-dmg.bundle"
MAX_DMG_BYTES=$((300 * 1024 * 1024))

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BRANCH="$(git -C "$ROOT" rev-parse --abbrev-ref HEAD)"
SHA="$(git -C "$ROOT" rev-parse --short HEAD)"
DEST="$ROOT/build/mac"
mkdir -p "$DEST"

BUNDLE_LOCAL="$DEST/ship-$SHA.bundle"
trap 'rm -f "$BUNDLE_LOCAL"' EXIT
git -C "$ROOT" bundle create "$BUNDLE_LOCAL" "$BRANCH" >/dev/null 2>&1
rsync -t "$BUNDLE_LOCAL" "$HOST:$BUNDLE_REMOTE"

ssh "$HOST" "set -e
export PATH=/opt/homebrew/bin:\$PATH
D=\$HOME/$REMOTE_DIR
if [ ! -d \$D/.git ]; then git clone -q $BUNDLE_REMOTE \$D; fi
cd \$D
git fetch -q $BUNDLE_REMOTE $BRANCH
git checkout -q -f --detach FETCH_HEAD
git log --oneline -1"

LOG="$DEST/ship-$SHA.log"
if ! ssh "$HOST" "cd \$HOME/$REMOTE_DIR && ./scripts/mac/build-dmg.sh" > "$LOG" 2>&1; then
  tail -40 "$LOG"
  echo "build failed, full log $LOG"
  exit 1
fi

REMOTE_OUT="$(ssh "$HOST" "ls -dt \$HOME/$REMOTE_DIR/build/mac/dmg-* | head -1")"
NAME="$(basename "$REMOTE_OUT")"
mkdir -p "$DEST/$NAME"
DMG_REMOTE="$(ssh "$HOST" "cat $REMOTE_OUT/dmg-path.txt")"
rsync -t "$HOST:$REMOTE_OUT/build.log" "$HOST:$DMG_REMOTE.sha256" "$DEST/$NAME/"
SIZE="$(ssh "$HOST" "stat -f %z '$DMG_REMOTE'")"
echo "dmg size bytes: $SIZE"
echo "$SIZE" > "$DEST/$NAME/dmg-size-bytes.txt"
if [ "$SIZE" -lt "$MAX_DMG_BYTES" ]; then
  rsync -t "$HOST:$DMG_REMOTE" "$DEST/$NAME/"
  echo "dmg copied to $DEST/$NAME"
else
  echo "dmg stays on $HOST:$DMG_REMOTE (over 300 MB)"
fi
cat "$DEST/$NAME/"*.sha256

ssh "$HOST" "cd \$HOME/$REMOTE_DIR && ./scripts/mac/check-dmg.sh '$DMG_REMOTE'" | tee "$DEST/$NAME/check.log"
