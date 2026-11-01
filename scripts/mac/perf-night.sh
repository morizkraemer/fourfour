#!/usr/bin/env bash
# Mac-night perf batch: real WKWebView startup, scroll and analysis numbers on the 10k fixture, plus the backend suite.
# Run on the Mac from the repo root (ssh mbp, or a mac-queue entry):
#   scripts/mac/perf-night.sh [--size 10k] [--app /Applications/fourfour.app | --build] [--runs 3] [--expand-runs 5] [--dry-run]
# Writes build/mac/perf-<date>/: summary.md (pass/fail), backend.json, app-run-*.jsonl, screenshots, logs.
# The app runs with HOME pointed at a scratch folder, so its config, library and caches never touch the real ones.
# --dry-run: no app window. Prints the launch commands and checks everything else (works on Linux too).
set -euo pipefail

SIZE=10k
APP="/Applications/fourfour.app"
BUILD=0
RUNS=3
EXPAND_RUNS=5
DRY=0
APP_TIMEOUT=240
while [ $# -gt 0 ]; do
  case "$1" in
    --size) SIZE="$2"; shift 2 ;;
    --app) APP="$2"; shift 2 ;;
    --build) BUILD=1; shift ;;
    --runs) RUNS="$2"; shift 2 ;;
    --expand-runs) EXPAND_RUNS="$2"; shift 2 ;;
    --dry-run) DRY=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
OUT="${FF_PERF_NIGHT_OUT:-$ROOT/build/mac/perf-$(date +%Y-%m-%d-%H%M)}"
mkdir -p "$OUT"
say() { echo "[perf-night] $*" | tee -a "$OUT/night.log" >&2; }

if [ "$DRY" = 0 ] && [ "$(uname -s)" != Darwin ]; then
  echo "perf-night.sh measures WKWebView and runs on macOS only (use --dry-run elsewhere)" >&2
  exit 2
fi
for tool in python3 node ffmpeg cargo; do
  command -v "$tool" >/dev/null || { say "missing $tool"; exit 1; }
done
say "out: $OUT  size: $SIZE  head: $(git -C "$ROOT" rev-parse --short HEAD)  host: $(uname -sm)"

if [ "$BUILD" = 1 ]; then
  say "building the app bundle (release, unsigned)"
  if [ "$DRY" = 1 ]; then
    say "+ (cd app && npm ci && npm run build) && (cd pioneer-test-ui && cargo tauri build --bundles app)"
  else
    (cd "$ROOT/app" && npm ci && npm run build) >> "$OUT/build.log" 2>&1
    (cd "$ROOT/pioneer-test-ui" && cargo tauri build --bundles app) >> "$OUT/build.log" 2>&1
  fi
  APP="$TARGET/release/bundle/macos/fourfour.app"
fi
BIN="$APP/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist" 2>/dev/null || echo fourfour)"
if [ "$DRY" = 0 ] && [ ! -x "$BIN" ]; then
  say "no app binary at $BIN (pass --app or --build)"
  exit 1
fi

say "backend suite and fixture: scripts/perf.sh --size $SIZE (frontend skipped: the headless numbers come from the box)"
FF_PERF_SKIP="frontend${FF_PERF_NIGHT_SKIP:+,$FF_PERF_NIGHT_SKIP}" FF_PERF_OUT="$OUT/backend.json" \
  nice -n 10 "$ROOT/scripts/perf.sh" --size "$SIZE" > "$OUT/backend.txt" 2> "$OUT/backend.log" ||
  { tail -n 30 "$OUT/backend.log" >&2; exit 1; }
FIXTURE="${FF_FIXTURE_ROOT:-$HOME/.cache/ff-fixture}/$SIZE"
[ -f "$FIXTURE/library/library.db" ] || { say "fixture missing at $FIXTURE"; exit 1; }

shot() {
  if [ "$DRY" = 1 ]; then say "+ screencapture -x $OUT/$1.png"; else screencapture -x "$OUT/$1.png" || say "screenshot $1 failed (screen recording permission?)"; fi
}

wait_mark() {
  local log="$1" mark="$2" deadline=$((SECONDS + APP_TIMEOUT))
  while [ $SECONDS -lt $deadline ]; do
    grep -q "\"mark\":\"$mark\"" "$log" 2>/dev/null && return 0
    sleep 0.5
  done
  return 1
}

app_run() {
  local n="$1" probe="$2" home="$OUT/home-$1"
  shift 2
  local data="$home/Library/Application Support/com.pioneer.test-ui"
  rm -rf "$home"
  mkdir -p "$data"
  cp -R "$FIXTURE/library" "$home/fixture-library"
  printf '{"library_path":"%s"}\n' "$home/fixture-library/library.db" > "$data/config.json"
  local log="$OUT/app-run-$n.jsonl"
  : > "$log"
  local env_args=(HOME="$home" FF_PERF_LOG="$log" FF_PERF_PROBE="$probe" FOURFOUR_MODELS_DIR="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}" "$@")
  if [ "$DRY" = 1 ]; then
    say "+ env ${env_args[*]} '$BIN'"
    return 0
  fi
  say "app run $n: $probe"
  env "${env_args[@]}" "$BIN" > "$OUT/app-run-$n.log" 2>&1 &
  local pid=$!
  if wait_mark "$log" interactive; then shot "run-$n-interactive"; else say "run $n: no interactive mark within ${APP_TIMEOUT}s"; fi
  if [[ "$probe" == *'"analyse":true'* ]]; then
    wait_mark "$log" scroll_analysis_start && sleep 3 && shot "run-$n-scroll-during-analysis"
  fi
  wait_mark "$log" probe_done || say "run $n: no probe_done mark within ${APP_TIMEOUT}s"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  sleep 2
}

say "first launch on a fresh copy, then warm launches"
app_run 1 '{"trackLoad":false,"scroll":false}'
for n in $(seq 2 "$RUNS"); do app_run "$n" '{"trackLoad":true,"scroll":true}'; done
say "cold first expand: $EXPAND_RUNS launches, one expand each (R10; over 50 ms longest frame triggers the staged reveal)"
for n in $(seq 1 "$EXPAND_RUNS"); do app_run "expand-$n" '{"trackLoad":false,"scroll":false,"expand":true}'; done
ANALYSIS_PROBE='{"trackLoad":false,"scroll":true,"analyse":true,"analysisWarmupMs":8000,"analysisScrollSeconds":10,"analysisBursts":5}'
CORES="$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)"
say "scroll during analysis: before R5 (no UI pause, $((CORES - 1)) threads), R5 default, R5 with background QoS"
app_run analysis-before "$ANALYSIS_PROBE" FF_ANALYSIS_UI_PAUSE=off FOURFOUR_ANALYSIS_THREADS="$((CORES - 1))"
app_run analysis "$ANALYSIS_PROBE"
app_run analysis-bgqos "$ANALYSIS_PROBE" FF_ANALYSIS_QOS=background

say "analysis throughput cost of the UI headroom (bench_analysis, 12 long fixture files each)"
BENCH="$TARGET/release/bench_analysis"
head -n 12 "${FF_FIXTURE_ROOT:-$HOME/.cache/ff-fixture}/$SIZE/long.txt" > "$OUT/headroom-list.txt"
headroom() {
  local name="$1" envs="$2" args="$3"
  if [ "$DRY" = 1 ]; then say "+ env $envs $BENCH --list $OUT/headroom-list.txt --limit 12 $args"; return 0; fi
  env FOURFOUR_MODELS_DIR="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}" $envs "$BENCH" --list "$OUT/headroom-list.txt" --limit 12 $args > "$OUT/headroom-$name.txt" 2> "$OUT/headroom-$name.log" ||
    say "headroom $name failed"
}
headroom before-threads "" "--threads $((CORES - 1))"
headroom default "" ""
headroom ui-busy-1-in-3 "" "--ui-busy 2000/6000"
headroom bgqos "FF_ANALYSIS_QOS=background" ""

node "$ROOT/scripts/mac/perf-night-summary.mjs" "$OUT" "$SIZE" | tee "$OUT/summary.md"
say "done: $OUT"
