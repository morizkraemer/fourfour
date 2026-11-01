#!/usr/bin/env bash
# Long-session memory soak: the frontend loop (app/smoke/soak.spec.ts, 10k synthetic tracks) and the backend loop
# (bench_startup soak). Prints the samples and writes JSON reports.
# Usage: scripts/soak.sh [minutes]   (default 45; FF_SOAK_BACKEND_SECONDS defaults to the same length)
# Env: FF_SOAK_SAMPLE_SECS (180), FF_SOAK_SNAPSHOT_DIR (heap snapshots of the first and last sample, diff them with
# `node app/perf/heap-diff.mjs first.heapsnapshot last.heapsnapshot`), FF_SOAK_ARTWORK=1 (tracks have covers),
# FF_PERF_DIR (fixtures, default ~/.cache/q50-perf), FF_SMOKE_PORT (5381), FF_SOAK_OUT (frontend report),
# FF_SOAK_SKIP (comma list of: frontend,backend).
set -euo pipefail

MINUTES="${1:-45}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
DIR="${FF_PERF_DIR:-$HOME/.cache/q50-perf}"
PORT="${FF_SMOKE_PORT:-5381}"
SKIP=",${FF_SOAK_SKIP:-},"
say() { echo "[soak] $*" >&2; }
skip() { [[ "$SKIP" == *",$1,"* ]]; }

if ! skip backend; then
  cargo build --release --quiet --bin bench_startup --manifest-path "$ROOT/Cargo.toml"
  LIB="$DIR/lib10000"
  if [ ! -f "$LIB/library.db" ]; then
    say "generating a 10000-track library in $LIB (one time)"
    "$TARGET/release/bench_startup" gen "$LIB" 10000 2>/dev/null
  fi
  say "backend: library reads, analysis stores, playlist saves and detail-cache writes for ${FF_SOAK_BACKEND_SECONDS:-$((MINUTES * 60))} s"
  "$TARGET/release/bench_startup" soak "$LIB" "${FF_SOAK_BACKEND_SECONDS:-$((MINUTES * 60))}"
fi

if ! skip frontend; then
  say "frontend: $MINUTES min of scrolling, search, palette, playlists, load and seek, settings and analysis events"
  (cd "$ROOT/app" && FF_SOAK_MINUTES="$MINUTES" FF_PERF_TRACKS=10000 FF_SMOKE_PORT="$PORT" npx playwright test soak --reporter=list)
fi
