#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-$HOME/dev/build/fourfour-r140}"
HERE="$(cd "$(dirname "$0")" && pwd)"
RUN="caffeinate -d -i python3 -I $HERE/r140-soak.py --root $ROOT run"
for n in 1 2 3; do $RUN "scroll$n" scroll --analysis-ms 60000 --idle-after 120; done
for n in 1 2 3; do $RUN "expand$n" expand --idle-after 60; done
$RUN soak1 soak --soak-minutes 25 --idle-after 120
python3 -I "$HERE/r140-soak.py" --root "$ROOT" summary
