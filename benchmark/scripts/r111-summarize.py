"""usage: r111-load-state.sh pairs ... | r111-summarize.py

Reads the lines `<label> REP runs=N bytes=B name=median (min-max) ...` and prints, per label and metric, the median of
the per-process medians, the spread of those medians, and the lowest minimum and highest maximum of all runs.
"""
import re
import statistics
import sys
from collections import defaultdict

metrics = defaultdict(lambda: defaultdict(list))
for line in sys.stdin:
    label, _, rest = line.partition(" ")
    for name, median, low, high in re.findall(r"(\w+_ms)=([\d.]+) \(([\d.]+)-([\d.]+)\)", rest):
        metrics[label][name].append((float(median), float(low), float(high)))
for label, by_name in metrics.items():
    for name, values in by_name.items():
        medians = [v[0] for v in values]
        print(
            f"{label:8} {name:16} median {statistics.median(medians):7.1f}  "
            f"medians {min(medians):.1f}-{max(medians):.1f}  fastest run {min(v[1] for v in values):.1f}  "
            f"slowest run {max(v[2] for v in values):.1f}  ({len(values)} processes)"
        )
