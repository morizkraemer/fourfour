#!/usr/bin/env bash
# Q15 pipeline, batched the way Q15 ran it: 4 bgdump processes with 3 rten threads each over the 50-track set.
set -u
here="$(cd "$(dirname "$0")/../.." && pwd)"
out="$here/benchmark/results/q21/before"
mkdir -p "$out/bg4"
start=$(date +%s.%N)
for i in 0 1 2 3; do
  RTEN_NUM_THREADS=3 /usr/bin/time -v -o "$out/bg4-$i.time" "$here/benchmark/results/q21/before-bin/bgdump" \
    --reference "$here/benchmark/results/q21/ref50.json" --out "$out/bg4" --shard "$i/4" > "$out/bg4-$i.out" 2> "$out/bg4-$i.err" &
done
wait
end=$(date +%s.%N)
echo "wall $(echo "$end - $start" | bc) s"
grep -h "Maximum resident" "$out"/bg4-*.time
