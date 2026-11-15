#!/usr/bin/env bash
set -uo pipefail
RUNS=${1:?runs}
BROWSER=${2:-chromium}
BUSY=${FF_LOAD_BUSY:-20}
PORT=${FF_SMOKE_PORT:-5691}
OUT=${FF_LOAD_OUT:-/tmp/ff-load-run}
mkdir -p "$OUT"
pids=()
for _ in $(seq "$BUSY"); do
  node -e 'for(;;);' &
  pids+=($!)
done
trap 'kill "${pids[@]}" 2>/dev/null' EXIT
sleep 20
for i in $(seq "$RUNS"); do
  log="$OUT/$BROWSER-$i.log"
  FF_SMOKE_BROWSER=$BROWSER FF_SMOKE_PORT=$PORT npx playwright test >"$log" 2>&1
  status=$?
  echo "run $i $BROWSER exit $status $(grep -E '^smoke: status' "$log")"
  grep -E '^(FAILED|FLAKY) ' "$log"
done
