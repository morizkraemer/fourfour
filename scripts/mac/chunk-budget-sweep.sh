#!/usr/bin/env bash
# R61: analysis throughput and peak RSS against the beat-model chunk budget (FOURFOUR_CHUNK_BUDGET_MB).
# Run on a quiet Mac from the repo root:
#   scripts/mac/chunk-budget-sweep.sh [--n 200] [--budgets "unset 1024 2048 3072 4096"] [--threads T] [--fixture 10k]
#                                     [--list FILE] [--out DIR] [--no-build] [--dry-run]
# Takes real-length tracks of the fixture (the long mixes first, then every file over 2 MB), not the 8 s ones.
# Writes DIR/sweep.tsv and DIR/summary.md (scripts/mac/chunk-budget-summary.py), plus one log per budget.
set -euo pipefail

N=200
BUDGETS="unset 1024 2048 3072 4096"
THREADS=""
FIXTURE=10k
LIST=""
OUT=""
BUILD=1
DRY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --n) N="$2"; shift 2 ;;
    --budgets) BUDGETS="$2"; shift 2 ;;
    --threads) THREADS="$2"; shift 2 ;;
    --fixture) FIXTURE="$2"; shift 2 ;;
    --list) LIST="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --no-build) BUILD=0; shift ;;
    --dry-run) DRY=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
BENCH="$TARGET/release/bench_analysis"
FIX="${FF_FIXTURE_ROOT:-$HOME/.cache/ff-fixture}/$FIXTURE"
MODELS="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}"
OUT="${OUT:-$ROOT/build/mac/chunk-budget-$(date +%Y-%m-%d-%H%M)}"
mkdir -p "$OUT"
say() { echo "[chunk-budget] $*" | tee -a "$OUT/sweep.log" >&2; }

TIME=(/usr/bin/time -l)
[ "$(uname -s)" = Darwin ] || TIME=(/usr/bin/time -v)

if [ -z "$LIST" ]; then
  LIST="$OUT/tracks.txt"
  { cat "$FIX/long.txt"; find "$FIX/music" -type f -size +2M | LC_ALL=C sort; } | awk -v n="$N" '!seen[$0]++ && c < n { print; c++ }' > "$LIST"
fi
COUNT="$(wc -l < "$LIST" | tr -d ' ')"
if [ "$COUNT" -lt "$N" ]; then
  echo "only $COUNT real-length tracks available, wanted $N" >&2
  exit 2
fi
say "$COUNT tracks from $LIST, budgets: $BUDGETS"

if [ "$BUILD" = 1 ]; then
  say "building release bench_analysis"
  (cd "$ROOT" && cargo build --release --quiet --features pioneer-test-ui/dev-bins --bin bench_analysis)
fi

THREAD_ARGS=()
[ -n "$THREADS" ] && THREAD_ARGS=(--threads "$THREADS")

TSV="$OUT/sweep.tsv"
printf 'budget_mb\ttracks\terrors\twall_s\ttracks_per_min\tpeak_rss_mb\ttree_rss_mb\tcpu_s\texit\n' > "$TSV"

for budget in $BUDGETS; do
  ENVV=(env FOURFOUR_MODELS_DIR="$MODELS")
  if [ "$budget" = unset ]; then
    ENVV+=(-u FOURFOUR_CHUNK_BUDGET_MB)
  else
    ENVV+=(FOURFOUR_CHUNK_BUDGET_MB="$budget")
  fi
  say "budget $budget: $COUNT tracks"
  if [ "$DRY" = 1 ]; then
    say "+ ${TIME[*]} -o $OUT/time-$budget.txt ${ENVV[*]} $BENCH --list $LIST --limit $N ${THREAD_ARGS[@]+"${THREAD_ARGS[@]}"}"
    continue
  fi
  status=0
  "${TIME[@]}" -o "$OUT/time-$budget.txt" "${ENVV[@]}" "$BENCH" --list "$LIST" --limit "$N" ${THREAD_ARGS[@]+"${THREAD_ARGS[@]}"} > "$OUT/run-$budget.out" 2> "$OUT/run-$budget.log" || status=$?
  python3 -I "$ROOT/scripts/mac/chunk-budget-summary.py" row "$budget" "$status" "$OUT/run-$budget.out" "$OUT/time-$budget.txt" >> "$TSV"
done

if [ "$DRY" = 0 ]; then
  python3 -I "$ROOT/scripts/mac/chunk-budget-summary.py" table "$TSV" | tee "$OUT/summary.md"
fi
say "done: $OUT"
