#!/usr/bin/env bash
# R11: the beat model on CoreML (onnxruntime + CoreML EP) vs rten on the Mac, same binary, same tracks.
# Run on the Mac from the repo root, headless (no window, no screen access):
#   scripts/mac/coreml-bench.sh [--o3 DIR] [--split dev|test] [--limit N] [--reps 2] [--rten "W T"] [--coreml "W T;W T"]
#                               [--rounds 7] [--background] [--no-build] [--dry-run]
# --o3 DIR holds ref.json, dev.txt and test.txt with paths valid on this machine (default ~/.cache/r11-work/o3).
# --rten / --coreml: "workers threads" configs, several separated by ";". Runs are interleaved, rten first.
# --background: everything under nice -n 10 and taskpolicy -b (for a daytime check; numbers are then not representative).
# Writes build/mac/coreml-<date>/: summary.md, per-run jsonl, logs, logit dumps.
set -euo pipefail

O3="$HOME/.cache/r11-work/o3"
SPLIT=dev
LIMIT=0
REPS=2
RTEN_CFGS="2 6"
COREML_CFGS="2 1;2 2;4 2"
ROUNDS=7
BACKGROUND=0
BUILD=1
DRY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --o3) O3="$2"; shift 2 ;;
    --split) SPLIT="$2"; shift 2 ;;
    --limit) LIMIT="$2"; shift 2 ;;
    --reps) REPS="$2"; shift 2 ;;
    --rten) RTEN_CFGS="$2"; shift 2 ;;
    --coreml) COREML_CFGS="$2"; shift 2 ;;
    --rounds) ROUNDS="$2"; shift 2 ;;
    --background) BACKGROUND=1; shift ;;
    --no-build) BUILD=0; shift ;;
    --dry-run) DRY=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FFA_DIR="$ROOT/prototyping/analysis-packaging"
FFA_TARGET="${FFA_TARGET_DIR:-$FFA_DIR/target}"
FFA="$FFA_TARGET/release/ffa"
MODELS="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}"
OUT="${FF_COREML_OUT:-$ROOT/build/mac/coreml-$(date +%Y-%m-%d-%H%M)}"
mkdir -p "$OUT"
say() { echo "[coreml-bench] $*" | tee -a "$OUT/bench.log" >&2; }
run() {
  if [ "$DRY" = 1 ]; then say "+ $*"; else "$@"; fi
}
PRIO=()
if [ "$BACKGROUND" = 1 ]; then
  PRIO=(nice -n 10)
  command -v taskpolicy >/dev/null && PRIO+=(taskpolicy -b)
fi
TIME=(/usr/bin/time -l)
[ "$(uname -s)" = Darwin ] || TIME=(/usr/bin/time -v)

if [ "$DRY" = 0 ] && [ "$(uname -s)" != Darwin ]; then
  echo "coreml-bench.sh measures CoreML and runs on macOS only (use --dry-run elsewhere)" >&2
  exit 2
fi
for f in "$MODELS/beat_this.onnx" "$MODELS/mel_spectrogram.onnx" "$O3/ref.json" "$O3/$SPLIT.txt"; do
  [ -f "$f" ] || { say "missing $f"; exit 1; }
done
LIST="$OUT/$SPLIT.txt"
if [ "$LIMIT" -gt 0 ]; then head -n "$LIMIT" "$O3/$SPLIT.txt" > "$LIST"; else cp "$O3/$SPLIT.txt" "$LIST"; fi
missing=$(while IFS= read -r p; do [ -f "$p" ] || echo "$p"; done < "$LIST" | wc -l | tr -d ' ')
[ "$DRY" = 1 ] || [ "$missing" = 0 ] || { say "$missing tracks of $LIST are missing"; exit 1; }
say "out: $OUT  head: $(git -C "$ROOT" rev-parse --short HEAD)  host: $(uname -sm) $(sysctl -n machdep.cpu.brand_string 2>/dev/null || true)"
say "tracks: $(wc -l < "$LIST" | tr -d ' ') ($SPLIT)  reps: $REPS  rten: $RTEN_CFGS  coreml: $COREML_CFGS  background: $BACKGROUND"

if [ "$BUILD" = 1 ]; then
  say "building ffa --features coreml (first build downloads ONNX Runtime from cdn.pyke.io)"
  run ${PRIO[@]+"${PRIO[@]}"} env CARGO_TARGET_DIR="$FFA_TARGET" cargo build --release --manifest-path "$FFA_DIR/Cargo.toml" --features coreml --bin ffa \
    >> "$OUT/build.log" 2>&1
fi
[ "$DRY" = 1 ] || [ -x "$FFA" ] || { say "no ffa at $FFA"; exit 1; }

say "per chunk: one 1500-frame chunk, one thread, median of $ROUNDS (rten vs coreml in one process)"
run ${PRIO[@]+"${PRIO[@]}"} env FOURFOUR_MODELS_DIR="$MODELS" FF_CHUNK_ROUNDS="$ROUNDS" \
  cargo test --release --manifest-path "$ROOT/Cargo.toml" -p fourfour-analysis-rs --features coreml --lib \
  ort_tests -- --ignored --nocapture > "$OUT/chunk.txt" 2> "$OUT/chunk.err" || say "chunk test failed, see chunk.err"

one_run() {
  local runtime="$1" workers="$2" threads="$3" rep="$4"
  local name="$runtime-w$workers-t$threads-r$rep"
  local env_rt=()
  [ "$runtime" = rten ] && env_rt=(FOURFOUR_BEAT_RUNTIME=rten)
  local dump=()
  [ "$rep" = 1 ] && dump=(--dump "$OUT/dump-$runtime-w$workers-t$threads")
  say "run $name"
  run env ${env_rt[@]+"${env_rt[@]}"} ORT_LOG=info ${PRIO[@]+"${PRIO[@]}"} "${TIME[@]}" "$FFA" --models "$MODELS" --workers "$workers" --threads "$threads" \
    --reference "$O3/ref.json" --list "$LIST" ${dump[@]+"${dump[@]}"} --out "$OUT/$name.jsonl" 2> "$OUT/$name.err" ||
    say "run $name failed, see $name.err"
}

for rep in $(seq 1 "$REPS"); do
  IFS=';' read -ra rcfgs <<< "$RTEN_CFGS"
  IFS=';' read -ra ccfgs <<< "$COREML_CFGS"
  for cfg in "${rcfgs[@]}"; do read -r w t <<< "$cfg"; one_run rten "$w" "$t" "$rep"; done
  for cfg in "${ccfgs[@]}"; do read -r w t <<< "$cfg"; one_run coreml "$w" "$t" "$rep"; done
done

if [ "$DRY" = 1 ]; then
  say "+ python3 -I scripts/mac/coreml-bench-summary.py $OUT $O3/ref.json > $OUT/summary.md"
  exit 0
fi
python3 -I "$ROOT/scripts/mac/coreml-bench-summary.py" "$OUT" "$O3/ref.json" "$ROOT/benchmark/scripts/o3-score.py" > "$OUT/summary.md"
cat "$OUT/summary.md"
