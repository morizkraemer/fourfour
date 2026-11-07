#!/usr/bin/env bash
# R108: interleaved sweep of parallel tracks (workers) x threads over one track list.
# usage: r108-sweep.sh <reps> <list> <results.jsonl> <config>...
# A config is  label:binary:cores:threads:workers[:extra[:memory_mb]]
#   binary   analyze-json (one batch through the pool) or bench_analysis (the app's staged scheduler)
#   cores    "-" for all, or a taskset list such as 0-7
#   threads  "-" for the default of the binary, else --threads
#   workers  "-" for the default plan, else --workers (analyze-json) / FOURFOUR_ANALYSIS_WORKERS (bench_analysis)
#   extra    extra arguments joined by "+", e.g. --unstaged
#   memory_mb  simulated machine memory (FOURFOUR_ANALYSIS_MEMORY_MB)
# Each repetition runs every config once, in a rotated order, so a slow stretch of the box hits all of them.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
reps="$1"; list="$2"; out="$3"; shift 3
configs=("$@")
n=${#configs[@]}
for ((rep = 0; rep < reps; rep++)); do
  for ((k = 0; k < n; k++)); do
    IFS=: read -r label bin cores threads workers extra memory <<< "${configs[$(((k + rep) % n))]}:::"
    memory="${memory%%:*}"
    args=()
    [ "$threads" != "-" ] && args+=(--threads "$threads")
    runopts=()
    [ "$cores" != "-" ] && runopts+=(--cores "$cores")
    [ -n "$memory" ] && runopts+=(--env "FOURFOUR_ANALYSIS_MEMORY_MB=$memory")
    if [[ "$bin" == *bench_analysis ]]; then
      args+=(--limit 1000)
      [ "$workers" != "-" ] && runopts+=(--env "FOURFOUR_ANALYSIS_WORKERS=$workers")
    else
      [ "$workers" != "-" ] && args+=(--workers "$workers")
    fi
    [ -n "$extra" ] && IFS=+ read -r -a more <<< "$extra" && args+=("${more[@]}")
    python3 -I "$here/r108-run.py" --label "$label" --out "$out" "${runopts[@]+"${runopts[@]}"}" -- nice -n 5 "$bin" "${args[@]}" --list "$list" > /dev/null
  done
done
