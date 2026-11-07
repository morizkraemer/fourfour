#!/usr/bin/env python3
"""R108: run one analysis batch and append one JSON line of throughput numbers.

usage: r108-run.py --label L --out results.jsonl [--cores 0-7] [--env K=V ...] [--json-out file] -- <command...>

The command is `analyze-json ... --list tracks.txt` (progress lines `[i/N] path` on stderr, JSON on stdout) or
`bench_analysis ...` (`BENCH ...` line on stdout). Measured: wall, user and system CPU of the whole process tree,
peak RSS (largest child), time to the first finished track, finish times of every track, load average before and
after, sha256 of stdout for analyze-json (the analysis output, to prove two builds agree).
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import threading
import time


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--cores")
    ap.add_argument("--env", action="append", default=[])
    ap.add_argument("--json-out")
    ap.add_argument("--min-available-mb", type=int, default=3000)
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    cmd = args.cmd[1:] if args.cmd and args.cmd[0] == "--" else args.cmd

    available = next(int(l.split()[1]) // 1024 for l in open("/proc/meminfo") if l.startswith("MemAvailable"))
    if available < args.min_available_mb:
        sys.exit(f"MemAvailable {available} MB < {args.min_available_mb}, not starting")

    env = dict(os.environ)
    for kv in args.env:
        k, v = kv.split("=", 1)
        env[k] = v
    full = (["taskset", "-c", args.cores] if args.cores else []) + cmd
    load0 = open("/proc/loadavg").read().split()[0]
    t0 = time.monotonic()
    proc = subprocess.Popen(full, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    done_at = []
    err_lines = []
    stdout_chunks = []
    hasher = hashlib.sha256()

    def read_err():
        for raw in proc.stderr:
            line = raw.decode(errors="replace").rstrip()
            if re.match(r"^\[\d+/\d+\] ", line):
                done_at.append(time.monotonic() - t0)
            else:
                err_lines.append(line)

    def read_out():
        while True:
            block = proc.stdout.read(1 << 20)
            if not block:
                break
            hasher.update(block)
            stdout_chunks.append(block)

    threads = [threading.Thread(target=read_err), threading.Thread(target=read_out)]
    for t in threads:
        t.start()
    _, status, usage = os.wait4(proc.pid, 0)
    wall = time.monotonic() - t0
    for t in threads:
        t.join()
    load1 = open("/proc/loadavg").read().split()[0]
    proc.returncode = os.waitstatus_to_exitcode(status)

    text = b"".join(stdout_chunks).decode(errors="replace")
    row = {
        "label": args.label,
        "cores": args.cores,
        "env": args.env,
        "cmd": " ".join(cmd),
        "rc": proc.returncode,
        "wall_s": round(wall, 2),
        "user_s": round(usage.ru_utime, 2),
        "sys_s": round(usage.ru_stime, 2),
        "maxrss_mb": round(usage.ru_maxrss / 1024, 0),
        "load_before": float(load0),
        "load_after": float(load1),
        "stdout_sha256": hasher.hexdigest(),
    }
    if done_at:
        row["tracks"] = len(done_at)
        row["first_s"] = round(done_at[0], 2)
        row["done_at_s"] = [round(x, 1) for x in done_at]
    bench = re.search(r"^BENCH .*$", text, re.M)
    if bench:
        row["bench"] = bench.group(0)
        for key in ("tracks", "tracks_per_min", "first_result", "peak_rss_tree", "wall"):
            m = re.search(rf"\b{key}=([\d.]+)", bench.group(0))
            if m:
                row["bench_" + key] = float(m.group(1))
    idle = [re.search(r"waited ([\d.]+) s", l) for l in err_lines if l.startswith("Beat model threads waited")]
    if idle:
        row["chunk_idle_s"] = float(idle[-1].group(1))
    beat = re.search(r"^BEAT .*chunk_idle_s=([\d.]+)", text, re.M)
    if beat:
        row["chunk_idle_s"] = float(beat.group(1))
    starting = [l for l in err_lines if "Starting" in l]
    if starting:
        row["starting"] = starting[-1]
    if args.json_out and proc.returncode == 0:
        with open(args.json_out, "wb") as f:
            f.write(b"".join(stdout_chunks))
    if proc.returncode != 0:
        row["stderr_tail"] = err_lines[-5:]
    n = row.get("tracks") or row.get("bench_tracks")
    if n:
        row["tracks_per_min"] = round(n / wall * 60, 2)
        row["cpu_per_track_s"] = round((usage.ru_utime + usage.ru_stime) / n, 2)
    with open(args.out, "a") as f:
        f.write(json.dumps(row) + "\n")
    print(json.dumps({k: v for k, v in row.items() if k not in ("done_at_s", "cmd", "stderr_tail", "env")}))


main()
