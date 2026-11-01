#!/usr/bin/env bash
# Perf suite: startup, track load, import, analysis, UI. Prints a table and writes one JSON report.
# Usage: scripts/perf.sh [--size 10k|25k|50k]
# With --size the suite runs on the real-size fixture from scripts/ff-fixture.py (generated once under
# ~/.cache/ff-fixture/<size>) and ends with the budget table from scripts/perf-budgets.json.
# Without it, the Q50 setup: a synthetic library from `bench_startup gen` and files from the dataset.
# Env: FF_PERF_DIR (fixtures, default ~/.cache/q50-perf), FF_PERF_AUDIO (audio dataset root),
# FF_PERF_TRACKS (library size, 10000), FF_PERF_IMPORT (files, 1000), FF_PERF_ANALYSIS (tracks, 6),
# FF_PERF_PROFILE_TRACKS (3), FF_PERF_THREADS (analysis threads, app default), FF_PERF_JOBS (fixture jobs, 12),
# FF_PERF_SKIP (comma list of: startup,load,import,analysis,playback,frontend), FF_SMOKE_PORT (5381), FF_PERF_OUT (report path),
# FF_FIXTURE_FULL (50k: real copies, about 35 GB, instead of hard links to the 25k fixture).
set -euo pipefail

SIZE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --size) SIZE="$2"; shift 2 ;;
    --size=*) SIZE="${1#--size=}"; shift ;;
    *) echo "unknown argument: $1 (usage: scripts/perf.sh [--size 10k|25k|50k])" >&2; exit 2 ;;
  esac
done
case "$SIZE" in ""|10k|25k|50k) ;; *) echo "--size must be 10k, 25k or 50k" >&2; exit 2 ;; esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAIN="$(cd "$(git -C "$ROOT" rev-parse --git-common-dir)/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
DIR="${FF_PERF_DIR:-$HOME/.cache/q50-perf}"
AUDIO="${FF_PERF_AUDIO:-$MAIN/benchmark/datasets}"
TRACKS="${FF_PERF_TRACKS:-10000}"
IMPORT="${FF_PERF_IMPORT:-1000}"
ANALYSIS="${FF_PERF_ANALYSIS:-6}"
PROFILE_TRACKS="${FF_PERF_PROFILE_TRACKS:-3}"
THREADS="${FF_PERF_THREADS:-}"
SKIP=",${FF_PERF_SKIP:-},"
PORT="${FF_SMOKE_PORT:-5381}"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${FF_PERF_OUT:-$MAIN/benchmark/results/q50/perf-${SIZE:+$SIZE-}$STAMP.json}"
RAW="$DIR/raw-${SIZE:+$SIZE-}$STAMP"
export FOURFOUR_MODELS_DIR="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}"

mkdir -p "$DIR" "$RAW" "$(dirname "$OUT")"
skip() { [[ "$SKIP" == *",$1,"* ]]; }
say() { echo "[perf] $*" >&2; }
bench() { "$TARGET/release/$1" "${@:2}"; }
perf_lines() { grep '^PERF ' | sed 's/^PERF //'; }

uptime | sed -E 's/.*load averages?: /load_before=/' > "$RAW/load.txt"
getconf _NPROCESSORS_ONLN >> "$RAW/load.txt"

say "building release benches"
cargo build --release --quiet --features pioneer-test-ui/dev-bins --bin bench_startup --bin bench_analysis --manifest-path "$ROOT/Cargo.toml"

if [ -n "$SIZE" ]; then
  say "fixture: $SIZE (generated once; see scripts/ff-fixture.py)"
  LINK=()
  if [ "$SIZE" = 50k ] && [ -z "${FF_FIXTURE_FULL:-}" ]; then
    python3 -I "$ROOT/scripts/ff-fixture.py" 25k --bench "$TARGET/release/bench_startup" --jobs "${FF_PERF_JOBS:-12}" > /dev/null
    LINK=(--link-from 25k)
  fi
  FIXTURE="$(python3 -I "$ROOT/scripts/ff-fixture.py" "$SIZE" ${LINK[@]+"${LINK[@]}"} --bench "$TARGET/release/bench_startup" --jobs "${FF_PERF_JOBS:-12}" | tail -n 1)"
  cp "$FIXTURE/fixture.json" "$RAW/fixture.json"
  LIB="$FIXTURE/library"
  TRACKS="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["files"])' "$FIXTURE/fixture.json")"
  IMPORT="$TRACKS"
  ANALYSIS="${FF_PERF_ANALYSIS:-12}"
  head -n 12 "$FIXTURE/long.txt" > "$DIR/audio-load-$SIZE.txt"
  head -n "$ANALYSIS" "$FIXTURE/long.txt" > "$DIR/audio-analysis-$SIZE.txt"
  LOAD_LIST="$DIR/audio-load-$SIZE.txt"
  ANALYSIS_LIST="$DIR/audio-analysis-$SIZE.txt"
else
  LIB="$DIR/lib$TRACKS"
  if [ ! -f "$LIB/library.db" ]; then
    say "generating a $TRACKS-track library in $LIB (one time)"
    bench bench_startup gen "$LIB" "$TRACKS" 2>/dev/null
  fi
  AUDIO_LIST="$DIR/audio-all.txt"
  find -L "$AUDIO" -type f \( -iname '*.mp3' -o -iname '*.flac' -o -iname '*.wav' -o -iname '*.m4a' -o -iname '*.aiff' \) | LC_ALL=C sort > "$AUDIO_LIST"
  [ -s "$AUDIO_LIST" ] || { echo "no audio under $AUDIO (set FF_PERF_AUDIO)" >&2; exit 1; }
  awk 'NR % 41 == 1 && ++n <= 12' "$AUDIO_LIST" > "$DIR/audio-load.txt"
  awk -v step="$(( $(wc -l < "$AUDIO_LIST") / ANALYSIS ))" -v max="$ANALYSIS" 'NR % step == 7 && ++n <= max' "$AUDIO_LIST" > "$DIR/audio-analysis.txt"
  LOAD_LIST="$DIR/audio-load.txt"
  ANALYSIS_LIST="$DIR/audio-analysis.txt"
fi

evict() {
  python3 -I - "$@" <<'PY'
import os, sys
if not hasattr(os, "posix_fadvise"):
    print("[perf] page cache eviction needs posix_fadvise (Linux); cold runs here are warm", file=sys.stderr)
    sys.exit(0)
for root in sys.argv[1:]:
    paths = [root] if os.path.isfile(root) else [os.path.join(d, f) for d, _, fs in os.walk(root) for f in fs]
    for p in paths:
        try:
            fd = os.open(p, os.O_RDONLY)
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
            os.close(fd)
        except OSError:
            pass
PY
}

if ! skip startup; then
  say "startup backend: cold (page cache evicted) then 3 warm"
  evict "$LIB"
  bench bench_startup run "$LIB" | perf_lines | sed 's/"startup_backend"/"startup_backend_cold"/' > "$RAW/startup_cold.jsonl"
  for _ in 1 2 3; do bench bench_startup run "$LIB"; done | perf_lines > "$RAW/startup_warm.jsonl"
fi

if ! skip load; then
  say "track load backend: get_analysis_data, decode start, waveform detail"
  bench bench_startup load "$LIB" 60 | perf_lines > "$RAW/load.jsonl"
  bench bench_startup audio "$LOAD_LIST" "$DIR/work" | perf_lines >> "$RAW/load.jsonl"
fi

if ! skip import; then
  if [ -n "$SIZE" ]; then
    say "import of the $TRACKS fixture files: into an empty library (cold, warm), then again into its own library (all already there)"
    FOLDER="$FIXTURE/music"
    INTO="$LIB/library.db"
  else
    say "import of $IMPORT real files: into an empty library and into the $TRACKS-track one"
    FOLDER="$DIR/import$IMPORT"
    INTO="$LIB/library.db"
    if [ "$(find "$FOLDER" -type l 2>/dev/null | wc -l)" != "$IMPORT" ]; then
      rm -rf "$FOLDER"
      mkdir -p "$FOLDER"
      head -n "$IMPORT" "$AUDIO_LIST" | awk -v dir="$FOLDER" '{ n = split($0, parts, "."); printf "%s\t%s/d%02d/f%05d.%s\n", $0, dir, NR % 40, NR, parts[n] }' |
        while IFS=$'\t' read -r src dst; do mkdir -p "$(dirname "$dst")"; ln -s "$src" "$dst"; done
    fi
  fi
  evict "$FOLDER"
  bench bench_startup import "$FOLDER" "$DIR/work" | perf_lines | sed 's/"import_backend"/"import_backend_empty_cold"/' > "$RAW/import.jsonl"
  bench bench_startup import "$FOLDER" "$DIR/work" | perf_lines | sed 's/"import_backend"/"import_backend_empty_warm"/' >> "$RAW/import.jsonl"
  bench bench_startup import "$FOLDER" "$DIR/work" "$INTO" | perf_lines | sed 's/"import_backend"/"import_backend_into_library"/' >> "$RAW/import.jsonl"
fi

if ! skip analysis; then
  say "analysis: per-stage profile of $PROFILE_TRACKS tracks, then throughput of $ANALYSIS (this takes minutes)"
  THREAD_ARGS=()
  [ -n "$THREADS" ] && THREAD_ARGS=(--threads "$THREADS")
  bench bench_analysis --list "$ANALYSIS_LIST" --limit "$ANALYSIS" --profile "$PROFILE_TRACKS" "${THREAD_ARGS[@]}" 2>"$RAW/analysis-profile.log" | perf_lines > "$RAW/analysis.jsonl"
  bench bench_analysis --list "$ANALYSIS_LIST" --limit "$ANALYSIS" "${THREAD_ARGS[@]}" 2>"$RAW/analysis-run.log" | perf_lines >> "$RAW/analysis.jsonl"
fi

if ! skip playback; then
  say "playback engine: load and seek to first audio (fake output device, release test build)"
  (cd "$ROOT" && cargo test --release --quiet -p pioneer-test-ui latency_load_and_seek_to_first_audio -- --nocapture 2>&1) | grep '^LATENCY' > "$RAW/playback.txt" || true
fi

if ! skip frontend; then
  STATE_ENV=()
  if [ -n "$SIZE" ]; then
    bench bench_startup dump "$LIB" "$RAW/state.json"
    STATE_ENV=(FF_PERF_STATE="$RAW/state.json")
  fi
  say "frontend: production build + smoke Tauri mock at $TRACKS tracks (startup, track load, import, steady-state UI)"
  (cd "$ROOT/app" && env "${STATE_ENV[@]}" FF_PERF_Q50=1 FF_PERF_TRACKS="$TRACKS" FF_PERF_IMPORT="$IMPORT" FF_SMOKE_PORT="$PORT" \
    FF_PERF_Q50_OUT="$RAW/frontend-q50.json" FF_PERF_OUT="$RAW/frontend-perf10k.json" \
    npx playwright test perf-q50 perf10k --grep 'startup, track load|measure' --reporter=line > "$RAW/frontend.log" 2>&1) ||
    { tail -n 40 "$RAW/frontend.log" >&2; exit 1; }
  say "frontend: UI responsiveness during bulk analysis (perf-bulk-analysis.spec.ts)"
  (cd "$ROOT/app" && FF_PERF_BULK=1 FF_PERF_TRACKS="$TRACKS" FF_SMOKE_PORT="$PORT" FF_PERF_OUT="$RAW/frontend-bulk.json" \
    npx playwright test perf-bulk-analysis --reporter=line > "$RAW/frontend-bulk.log" 2>&1) ||
    { tail -n 40 "$RAW/frontend-bulk.log" >&2; exit 1; }
fi

uptime | sed -E 's/.*load averages?: /load_after=/' >> "$RAW/load.txt"
node "$ROOT/scripts/perf-report.mjs" "$RAW" "$OUT" ${SIZE:+"$SIZE"}
say "report: $OUT (raw: $RAW)"
