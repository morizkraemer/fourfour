#!/usr/bin/env python3
"""Mac night 3, item 3: seconds from the start of a batch to the first finished track, bench_analysis stopped there.

usage: night3-first.py --label L --out first.jsonl [--env K=V ...] [--list FILE] [--limit N] [--wait-idle 0.8] -- <bench_analysis command>
Port of benchmark/scripts/r110-first.py for macOS (no /proc): idle share from `top`, load from os.getloadavg().
Appends one JSON line: label, first_s, load_before, idle_before, quiet (true when the idle share was reached).
"""
import argparse
import json
import os
import re
import shutil
import signal
import subprocess
import time


def idle_share():
    text = subprocess.run(["top", "-l", "2", "-s", "1", "-n", "0"], capture_output=True, text=True).stdout
    found = re.findall(r"CPU usage:.*?([\d.]+)% idle", text)
    return float(found[-1]) / 100 if found else 0.0


def wait_for_quiet(min_idle, max_wait):
    waited = 0.0
    while waited < max_wait:
        if idle_share() >= min_idle:
            return waited, True
        waited += 3
    return waited, False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--env", action="append", default=[])
    ap.add_argument("--list", required=True)
    ap.add_argument("--limit", type=int, default=24)
    ap.add_argument("--max-s", type=float, default=120)
    ap.add_argument("--wait-idle", type=float, default=0)
    ap.add_argument("--wait-max", type=float, default=60)
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    cmd = args.cmd[1:] if args.cmd and args.cmd[0] == "--" else args.cmd
    env = dict(os.environ)
    for kv in args.env:
        key, value = kv.split("=", 1)
        env[key] = value
    waited, quiet = wait_for_quiet(args.wait_idle, args.wait_max) if args.wait_idle else (0.0, True)
    idle = idle_share()
    load = os.getloadavg()[0]
    proc = subprocess.Popen(
        cmd + ["--verbose", "--list", args.list, "--limit", str(args.limit)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    began = time.monotonic()
    start = first = None
    for raw in proc.stderr:
        now = time.monotonic()
        line = raw.decode(errors="replace")
        if "Starting analysis" in line and start is None:
            start = now
        if re.search(r"\[ok\] \[1/\d+\]", line):
            first = now
            break
        if now - began > args.max_s:
            break
    os.killpg(proc.pid, signal.SIGKILL)
    proc.wait()
    tmp = env.get("TMPDIR", "")
    if tmp and os.path.isdir(tmp):
        for name in os.listdir(tmp):
            if name.startswith("bench-analysis-rust-"):
                shutil.rmtree(os.path.join(tmp, name), ignore_errors=True)
    row = {
        "label": args.label,
        "env": args.env,
        "first_s": round(first - start, 2) if first and start else None,
        "load_before": round(load, 2),
        "idle_before": round(idle, 2),
        "waited_s": waited,
        "quiet": quiet,
    }
    with open(args.out, "a") as out:
        out.write(json.dumps(row) + "\n")
    print(args.label, row["first_s"], "load", row["load_before"], "idle", row["idle_before"])


main()
