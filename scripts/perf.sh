#!/usr/bin/env bash
# Q50 perf suite: startup, track load, import, analysis, UI. Prints a table and writes one JSON report.
# Env: FF_PERF_DIR (fixtures, default ~/.cache/q50-perf), FF_PERF_AUDIO (audio dataset root),
# FF_PERF_TRACKS (library size, 10000), FF_PERF_IMPORT (files, 1000), FF_PERF_ANALYSIS (tracks, 6),
# FF_PERF_PROFILE_TRACKS (3), FF_PERF_SKIP (comma list of: startup,load,import,analysis,playback,frontend),
# FF_SMOKE_PORT (5381), FF_PERF_OUT (report path).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAIN="$(cd "$(git -C "$ROOT" rev-parse --git-common-dir)/.." && pwd)"
DIR="${FF_PERF_DIR:-$HOME/.cache/q50-perf}"
AUDIO="${FF_PERF_AUDIO:-$MAIN/benchmark/datasets}"
TRACKS="${FF_PERF_TRACKS:-10000}"
IMPORT="${FF_PERF_IMPORT:-1000}"
ANALYSIS="${FF_PERF_ANALYSIS:-6}"
PROFILE_TRACKS="${FF_PERF_PROFILE_TRACKS:-3}"
SKIP=",${FF_PERF_SKIP:-},"
PORT="${FF_SMOKE_PORT:-5381}"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${FF_PERF_OUT:-$MAIN/benchmark/results/q50/perf-$STAMP.json}"
RAW="$DIR/raw-$STAMP"
export FOURFOUR_MODELS_DIR="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}"

mkdir -p "$DIR" "$RAW" "$(dirname "$OUT")"
skip() { [[ "$SKIP" == *",$1,"* ]]; }
say() { echo "[perf] $*" >&2; }
bench() { "$ROOT/target/release/$1" "${@:2}"; }
perf_lines() { grep '^PERF ' | sed 's/^PERF //'; }

uptime | sed 's/.*load average: /load_before=/' > "$RAW/load.txt"
nproc >> "$RAW/load.txt"

say "building release benches"
cargo build --release --quiet --bin bench_startup --bin bench_analysis --manifest-path "$ROOT/Cargo.toml"

LIB="$DIR/lib$TRACKS"
if [ ! -f "$LIB/library.db" ]; then
  say "generating a $TRACKS-track library in $LIB (one time)"
  bench bench_startup gen "$LIB" "$TRACKS" 2>/dev/null
fi

AUDIO_LIST="$DIR/audio-all.txt"
find -L "$AUDIO" -type f \( -iname '*.mp3' -o -iname '*.flac' -o -iname '*.wav' -o -iname '*.m4a' -o -iname '*.aiff' \) | LC_ALL=C sort > "$AUDIO_LIST"
[ -s "$AUDIO_LIST" ] || { echo "no audio under $AUDIO (set FF_PERF_AUDIO)" >&2; exit 1; }
awk 'NR % 41 == 1' "$AUDIO_LIST" | head -n 12 > "$DIR/audio-load.txt"
awk -v step="$(( $(wc -l < "$AUDIO_LIST") / ANALYSIS ))" 'NR % step == 7' "$AUDIO_LIST" | head -n "$ANALYSIS" > "$DIR/audio-analysis.txt"

evict() {
  python3 -I - "$@" <<'PY'
import os, sys
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
  bench bench_startup audio "$DIR/audio-load.txt" "$DIR/work" | perf_lines >> "$RAW/load.jsonl"
fi

if ! skip import; then
  say "import of $IMPORT real files: into an empty library and into the $TRACKS-track one"
  FOLDER="$DIR/import$IMPORT"
  if [ "$(find "$FOLDER" -type l 2>/dev/null | wc -l)" != "$IMPORT" ]; then
    rm -rf "$FOLDER"
    mkdir -p "$FOLDER"
    head -n "$IMPORT" "$AUDIO_LIST" | awk -v dir="$FOLDER" '{ n = split($0, parts, "."); printf "%s\t%s/d%02d/f%05d.%s\n", $0, dir, NR % 40, NR, parts[n] }' |
      while IFS=$'\t' read -r src dst; do mkdir -p "$(dirname "$dst")"; ln -s "$src" "$dst"; done
  fi
  evict "$FOLDER"
  bench bench_startup import "$FOLDER" "$DIR/work" | perf_lines | sed 's/"import_backend"/"import_backend_empty_cold"/' > "$RAW/import.jsonl"
  bench bench_startup import "$FOLDER" "$DIR/work" | perf_lines | sed 's/"import_backend"/"import_backend_empty_warm"/' >> "$RAW/import.jsonl"
  bench bench_startup import "$FOLDER" "$DIR/work" "$LIB/library.db" | perf_lines | sed 's/"import_backend"/"import_backend_into_library"/' >> "$RAW/import.jsonl"
fi

if ! skip analysis; then
  say "analysis: per-stage profile of $PROFILE_TRACKS tracks, then throughput of $ANALYSIS (this takes minutes)"
  bench bench_analysis --list "$DIR/audio-analysis.txt" --limit "$ANALYSIS" --profile "$PROFILE_TRACKS" 2>"$RAW/analysis-profile.log" | perf_lines > "$RAW/analysis.jsonl"
  bench bench_analysis --list "$DIR/audio-analysis.txt" --limit "$ANALYSIS" 2>"$RAW/analysis-run.log" | perf_lines >> "$RAW/analysis.jsonl"
fi

if ! skip playback; then
  say "playback engine: load and seek to first audio (fake output device, release test build)"
  (cd "$ROOT" && cargo test --release --quiet -p pioneer-test-ui latency_load_and_seek_to_first_audio -- --nocapture 2>&1) | grep '^LATENCY' > "$RAW/playback.txt" || true
fi

if ! skip frontend; then
  say "frontend: production build + smoke Tauri mock at $TRACKS tracks (startup, track load, import, steady-state UI)"
  (cd "$ROOT/app" && FF_PERF_Q50=1 FF_PERF_TRACKS="$TRACKS" FF_PERF_IMPORT="$IMPORT" FF_SMOKE_PORT="$PORT" \
    FF_PERF_Q50_OUT="$RAW/frontend-q50.json" FF_PERF_OUT="$RAW/frontend-perf10k.json" \
    npx playwright test perf-q50 perf10k --grep 'startup, track load|measure' --reporter=line > "$RAW/frontend.log" 2>&1) ||
    { tail -n 40 "$RAW/frontend.log" >&2; exit 1; }
  say "frontend: UI responsiveness during bulk analysis (perf-bulk-analysis.spec.ts)"
  (cd "$ROOT/app" && FF_PERF_BULK=1 FF_PERF_TRACKS="$TRACKS" FF_SMOKE_PORT="$PORT" FF_PERF_OUT="$RAW/frontend-bulk.json" \
    npx playwright test perf-bulk-analysis --reporter=line > "$RAW/frontend-bulk.log" 2>&1) ||
    { tail -n 40 "$RAW/frontend-bulk.log" >&2; exit 1; }
fi

uptime | sed 's/.*load average: /load_after=/' >> "$RAW/load.txt"
node "$ROOT/scripts/perf-report.mjs" "$RAW" "$OUT"
say "report: $OUT (raw: $RAW)"
