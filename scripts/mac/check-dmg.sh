#!/bin/bash
set -uo pipefail

DMG="$1"
MOUNT="$(mktemp -d /tmp/fourfour-dmg-check.XXXXXX)"
failures=0

pass() { echo "PASS $1"; }
fail() { echo "FAIL $1"; failures=$((failures + 1)); }

check() {
  local label="$1"
  shift
  if "$@" >/dev/null 2>&1; then pass "$label"; else fail "$label"; fi
}

cleanup() {
  hdiutil detach "$MOUNT" -quiet 2>/dev/null || hdiutil detach "$MOUNT" -force -quiet 2>/dev/null
  rmdir "$MOUNT" 2>/dev/null
  echo "unmounted $MOUNT"
}
trap cleanup EXIT

hdiutil attach "$DMG" -nobrowse -noautoopen -readonly -mountpoint "$MOUNT" -quiet || { echo "FAIL mount"; exit 1; }
echo "mounted $DMG at $MOUNT"
ls "$MOUNT"

APP="$(ls -d "$MOUNT"/*.app | head -1)"
PLIST="$APP/Contents/Info.plist"
BIN="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")"
RES="$APP/Contents/Resources"
echo "app: $APP"

echo "--- codesign -dv"
SIGN="$(codesign -dv "$APP" 2>&1)"
echo "$SIGN"
if echo "$SIGN" | grep -q "not signed at all"; then
  pass "signature unsigned"
elif echo "$SIGN" | grep -q "Signature=adhoc"; then
  pass "signature ad-hoc"
else
  fail "signature neither unsigned nor ad-hoc"
fi
if echo "$SIGN" | grep -q "^Authority="; then fail "signature has an authority"; fi

check "weights beat_this_q8sc.onnx" test -s "$RES/models/beat_this_q8sc.onnx"
check "weights mel_spectrogram.onnx" test -s "$RES/models/mel_spectrogram.onnx"
check "licence LICENSE-beat-this.txt" test -s "$RES/models/LICENSE-beat-this.txt"
check "licence is MIT" grep -q "MIT License" "$RES/models/LICENSE-beat-this.txt"
check "licence LICENSE-key-model.txt" test -s "$RES/models/LICENSE-key-model.txt"
check "key model credits FSL10K" grep -q "Ramires" "$RES/models/LICENSE-key-model.txt"
check "key model credits FMA" grep -q "Defferrard" "$RES/models/LICENSE-key-model.txt"

echo "--- Info.plist"
for key in CFBundleIdentifier CFBundleShortVersionString CFBundleVersion CFBundleName; do
  value="$(/usr/libexec/PlistBuddy -c "Print :$key" "$PLIST" 2>/dev/null)"
  echo "$key=$value"
  if [ -n "$value" ]; then pass "plist $key"; else fail "plist $key empty"; fi
done

echo "--- self-check"
SELF="$("$BIN" --self-check 2>&1)"
code=$?
echo "$SELF"
if [ "$code" -eq 0 ]; then pass "self-check exit 0"; else fail "self-check exit $code"; fi

echo "failures=$failures"
[ "$failures" -eq 0 ]
