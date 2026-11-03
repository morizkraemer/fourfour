#!/usr/bin/env bash
# usage: r94-run.sh <label> <dev|test|list file> [VAR=value ...]   runs ffa (4 workers, 4 shared chunk threads, no key/waveform) on a split with the given
# beat-model switches (FOURFOUR_BEAT_CHUNK, FOURFOUR_BEAT_SKIP_SILENCE, MODEL=<file in $MODELS>), writes
# benchmark/results/r94/<label>.{jsonl,err,time} and prints the ffa SUMMARY, CPU seconds and the o3-score lines.
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
data="${O3:-/home/merlin/dev/projects/fourfour/benchmark/results/o3}"
ffa="${FFA:-/home/merlin/.cache/r94-target/release/ffa}"
models="${MODELS:-/home/merlin/.cache/r94-models}"
label="$1"; split="$2"; shift 2
case "$split" in dev|test) list="$data/$split.txt" ;; *) list="$split" ;; esac
model=beat_this.onnx
envs=()
for kv in "$@"; do
  case "$kv" in MODEL=*) model="${kv#MODEL=}" ;; *) envs+=("$kv") ;; esac
done
out="$here/benchmark/results/r94"
mkdir -p "$out"
avail=$(awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo)
if [ "$avail" -lt 1500 ]; then echo "$label: MemAvailable $avail MB < 1500, not starting" >&2; exit 3; fi
load0=$(cut -d' ' -f1 /proc/loadavg)
env "${envs[@]+"${envs[@]}"}" /usr/bin/time -f "cpu_user %U cpu_sys %S wall %e maxrss_kb %M" -o "$out/$label.time" \
  "$ffa" --models "$models" --beat-model "$model" --workers "${WORKERS:-4}" --threads "${THREADS:-4}" --no-key --no-waveform \
  --reference "$data/ref.json" --list "$list" --out "$out/$label.jsonl" 2> "$out/$label.err"
load1=$(cut -d' ' -f1 /proc/loadavg)
n=$(wc -l < "$out/$label.jsonl")
read -r user sys <<< "$(awk '{print $2, $4}' "$out/$label.time")"
echo "$label: $(grep SUMMARY "$out/$label.err" | sed 's/^SUMMARY //')"
echo "$label: cpu $(awk -v u="$user" -v s="$sys" -v n="$n" 'BEGIN {printf "%.1f s/track", (u+s)/n}'); load $load0 -> $load1"
python3 -I "$here/benchmark/scripts/o3-score.py" "$data/ref.json" "$out/$label.jsonl" | sed "s/^/$label: /"
