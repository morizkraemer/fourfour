#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-$HOME/dev/build/fourfour-r140}"
HERE="$(cd "$(dirname "$0")" && pwd)"
export FF_FIXTURE_ROOT="${FF_FIXTURE_ROOT:-$ROOT/fixture}"
for a in fling search palette playlists loadAndSeek settings analysis; do
  caffeinate -d -i python3 -I "$HERE/r140-soak.py" --root "$ROOT" run "wk-$a" soak --soak-minutes 7 --soak-actions "$a" --idle-after 360 --pressure-after 240 --max-seconds 1200
done
