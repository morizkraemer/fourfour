#!/usr/bin/env bash
# Old (Q15) non-beat stages on the first 3 tracks of the 50-track set: keydet detect, and ffproto --no-beats (decode + stratum + waveforms).
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
out="$here/benchmark/results/q21"
mapfile -t tracks < <(head -3 "$out/tracks50.txt")
"$here/benchmark/scripts/q21-timed.sh" "$out/old-keydet" "$out/before-bin/keydet" detect "${tracks[@]}"
"$here/benchmark/scripts/q21-timed.sh" "$out/old-ffproto" "$out/before-bin/ffproto" --no-beats "${tracks[@]}"
