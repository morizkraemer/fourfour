#!/usr/bin/env python3
"""R140: real-app memory and CPU on the Mac, one launch per run, scratch HOME.

  python3 -I r140-soak.py --root ~/dev/build/fourfour-r140 run NAME KIND [--idle-after S] [--soak-minutes M]
  python3 -I r140-soak.py --root ~/dev/build/fourfour-r140 summary

KIND: scroll (30 s idle, scripted scroll, analysis for --analysis-ms, idle after),
      expand (30 s idle, first expand of an analysed track, idle after),
      soak   (30 s idle, every soak action in a loop for --soak-minutes, idle after).
Needs <root>/fourfour.app (a copy of the bundle). Never touches Pioneer, rekordbox or the real app data;
the protected paths are hashed before and after every run. No audio is played, no USB is read.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

REAL_HOME = Path.home()
PROTECTED = [
    REAL_HOME / "Library/Application Support/Pioneer",
    REAL_HOME / "Library/Pioneer",
    REAL_HOME / "Library/Application Support/com.pioneer.test-ui",
    REAL_HOME / "Library/Application Support/rekordbox",
    REAL_HOME / "Library/Application Support/rekordbox6",
]
FIXTURE = Path(os.environ.get("FF_FIXTURE_ROOT", REAL_HOME / ".cache/ff-fixture")) / "10k"
MODELS = Path(os.environ.get("FOURFOUR_MODELS_DIR", REAL_HOME / ".cache/fourfour-models"))
SAMPLE_SECS = 2.0


def protected_hash():
    out = {}
    for p in PROTECTED:
        if not p.is_dir():
            out[str(p)] = "absent"
            continue
        rows = []
        for dirpath, _, files in os.walk(p):
            for f in files:
                fp = os.path.join(dirpath, f)
                try:
                    st = os.stat(fp)
                    rows.append(f"{fp} {st.st_size} {int(st.st_mtime)}")
                except OSError:
                    pass
        out[str(p)] = hashlib.sha256("\n".join(sorted(rows)).encode()).hexdigest()
    return out


def on_ac():
    text = subprocess.run(["pmset", "-g", "batt"], capture_output=True, text=True).stdout
    return "AC Power" in text


def cputime_secs(text):
    parts = text.split(":")
    total = 0.0
    for part in parts:
        total = total * 60 + float(part)
    return total


def process_table():
    out = subprocess.run(["ps", "-axo", "pid=,rss=,cputime=,command="], capture_output=True, text=True).stdout
    rows = {}
    for line in out.splitlines():
        bits = line.split(None, 3)
        if len(bits) < 4:
            continue
        try:
            rows[int(bits[0])] = (int(bits[1]), cputime_secs(bits[2]), bits[3])
        except ValueError:
            pass
    return rows


def webkit_pids(table):
    return {pid for pid, (_, _, cmd) in table.items() if "com.apple.WebKit." in cmd}


def footprint_mb(pid):
    out = subprocess.run(["footprint", "--noCategories", "-p", str(pid)], capture_output=True, text=True).stdout
    match = re.search(r"Footprint:\s*([\d.]+)\s*(KB|MB|GB)", out)
    if not match:
        return None
    return float(match.group(1)) * {"KB": 1 / 1024, "MB": 1, "GB": 1024}[match.group(2)]


def screen_locked():
    out = subprocess.run(["ioreg", "-n", "Root", "-d1", "-a"], capture_output=True, text=True).stdout
    return bool(re.search(r"CGSSessionScreenIsLocked</key>\s*<true/>", out))


def thin_library(db, unanalysed):
    con = sqlite3.connect(db)
    con.execute("pragma foreign_keys=on")
    con.execute("delete from tracks where id not in (select track_id from analyses)")
    con.execute(
        "delete from analyses where track_id in (select a.track_id from analyses a join tracks t on t.id = a.track_id "
        "where t.source_path not like '%Long Mixes%' order by a.track_id limit ? offset 1000)",
        (unanalysed,),
    )
    con.commit()
    total = con.execute("select count(*) from tracks").fetchone()[0]
    analysed = con.execute("select count(*) from analyses").fetchone()[0]
    con.execute("vacuum")
    con.close()
    return total, analysed


def analyses_done(db):
    try:
        con = sqlite3.connect(f"file:{db}?mode=ro", uri=True, timeout=2)
        n = con.execute("select count(*) from analyses where stages >= 7").fetchone()[0]
        con.close()
        return n
    except Exception:
        return None


def read_marks(log):
    marks = []
    try:
        for line in open(log):
            try:
                marks.append(json.loads(line))
            except ValueError:
                pass
    except OSError:
        pass
    return marks


def probe_for(kind, args):
    base = {"settleMs": 30000, "trackLoad": False, "scroll": False}
    if kind == "scroll":
        base.update({"scroll": True, "scrollSeconds": args.scroll_seconds, "analyse": True, "analysisWarmupMs": 500, "analysisScrollSeconds": 1, "analysisRunMs": args.analysis_ms})
    elif kind == "expand":
        base.update({"expand": True, "expandSettleMs": 1500})
    elif kind.startswith("backend"):
        return {}
    elif kind == "soak":
        ids = ",".join(str(i) for i in range(7, 10000, 250))
        base.update({"soakMinutes": args.soak_minutes, "soakSampleSecs": 30, "soakAnalysisIds": [int(i) for i in ids.split(",")]})
        if args.soak_actions:
            base["soakActions"] = args.soak_actions.split(",")
        if args.soak_steps:
            base["soakSteps"] = json.loads(args.soak_steps)
    return base


def run(args):
    root = Path(args.root).resolve()
    app = root / "fourfour.app"
    exe = app / "Contents/MacOS/pioneer-test-ui"
    out = root / "results" / args.name
    home = root / f"home-{args.name}"
    for bad in (home, out):
        s = str(bad).lower()
        assert str(bad).startswith(str(root) + "/"), bad
        assert "pioneer-usb" not in s and "rekordbox" not in s and "/library/" not in s, bad
    assert exe.is_file(), exe
    if not on_ac():
        sys.exit("BLOCKED: on battery")
    out.mkdir(parents=True, exist_ok=True)
    shutil.rmtree(home, ignore_errors=True)
    data = home / "Library/Application Support/com.pioneer.test-ui"
    data.mkdir(parents=True)
    (home / ".r140-scratch").write_text("scratch\n")
    shutil.copytree(FIXTURE / "library", home / "fixture-library", symlinks=True)
    db = home / "fixture-library/library.db"
    target = None
    if args.kind == "backend-analysis":
        total, analysed = thin_library(db, args.unanalysed)
        target = analysed + args.unanalysed
        print(f"thinned library: {total} tracks, {analysed} analysed, {args.unanalysed} to analyse")
    (data / "config.json").write_text(json.dumps({"library_path": str(db), "auto_analyze_new_tracks": args.kind == "backend-analysis"}))
    locked = screen_locked()
    if locked and not args.kind.startswith("backend"):
        shutil.rmtree(home, ignore_errors=True)
        sys.exit("BLOCKED: the Mac screen is locked, WKWebView stops animation frames and the probe never reaches the first row")
    before = protected_hash()
    probe = probe_for(args.kind, args)
    log = out / "marks.jsonl"
    log.write_text("")
    pre_webkit = webkit_pids(process_table())
    env = dict(os.environ, HOME=str(home), FF_PERF_LOG=str(log), FF_PERF_PROBE=json.dumps(probe), FOURFOUR_MODELS_DIR=str(MODELS))
    start_wall = time.time()
    proc = subprocess.Popen([str(exe)], env=env, stdout=open(out / "app.out", "w"), stderr=subprocess.STDOUT)
    samples = []
    prev = {}
    done_at = None
    next_footprint = 0
    footprints = []
    last_db = (None, 0)
    pressure = None
    deadline = start_wall + args.max_seconds
    while time.time() < deadline and proc.poll() is None:
        t = time.time()
        table = process_table()
        mine = {proc.pid} | (webkit_pids(table) - pre_webkit)
        row = {"t": round(t, 2), "app_rss_mb": 0.0, "app_cpu_s": 0.0, "web_rss_mb": 0.0, "web_cpu_s": 0.0, "web_procs": 0}
        for pid in mine:
            if pid not in table:
                continue
            rss, cpu, cmd = table[pid]
            if pid == proc.pid:
                row["app_rss_mb"], row["app_cpu_s"] = rss / 1024, cpu
            else:
                row["web_rss_mb"] += rss / 1024
                row["web_cpu_s"] += cpu
                row["web_procs"] += 1
                row.setdefault("web_names", []).append(cmd.split("/")[-1][:40])
        row["analysed"] = None
        if t - last_db[1] > 10:
            last_db = (analyses_done(db), t)
        row["analysed"] = last_db[0]
        if t >= next_footprint:
            fp = {pid: footprint_mb(pid) for pid in mine if pid in table}
            footprints.append({"t": round(t, 2), "app_mb": fp.get(proc.pid), "web_mb": sum(v or 0 for k, v in fp.items() if k != proc.pid), "per_pid": {str(k): [table[k][2].split("/")[-1][:40], v] for k, v in fp.items()}})
            next_footprint = t + args.footprint_secs
        samples.append(row)
        marks = read_marks(log)
        if done_at is None and args.kind == "backend-idle":
            opened = [m["wall_ms"] / 1000 for m in marks if m["mark"] == "load_state_encoded"]
            if opened and time.time() >= opened[0] + 30:
                done_at = time.time()
        elif done_at is None and args.kind == "backend-analysis":
            if last_db[0] is not None and target is not None and last_db[0] >= target:
                done_at = time.time()
        elif done_at is None and any(m["mark"] == "probe_done" for m in marks):
            done_at = time.time()
        if args.pressure_after and done_at is not None and pressure is None and time.time() - done_at >= args.pressure_after:
            pressure = subprocess.Popen(["memory_pressure", "-S", "-l", "warn", "-s", "30"], stdout=open(out / "pressure.out", "w"), stderr=subprocess.STDOUT)
            samples[-1]["pressure_start"] = True
        if done_at is not None and time.time() - done_at >= args.idle_after:
            break
        time.sleep(max(0.0, SAMPLE_SECS - (time.time() - t)))
    if pressure is not None and pressure.poll() is None:
        pressure.terminate()
    proc.terminate()
    try:
        proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        proc.kill()
    time.sleep(2)
    marks = read_marks(log)
    after = protected_hash()
    shutil.rmtree(home, ignore_errors=True)
    result = {
        "name": args.name, "kind": args.kind, "probe": probe, "start_wall": start_wall, "probe_done": done_at,
        "protected_unchanged": before == after, "screen_locked": locked, "start_db_target": target, "samples": samples, "footprints": footprints,
        "marks": [{"mark": m["mark"], "wall_ms": m["wall_ms"], "values": m["values"]} for m in marks if m["mark"] != "boot_timeline"],
    }
    (out / "run.json").write_text(json.dumps(result))
    print(f"{args.name}: {len(samples)} samples, probe_done={'yes' if done_at else 'NO'}, protected_unchanged={before == after}")


def mark_t(marks, name, last=False):
    hits = [m["wall_ms"] / 1000 for m in marks if m["mark"] == name]
    if not hits:
        return None
    return hits[-1] if last else hits[0]


def window(samples, a, b):
    return [s for s in samples if a is not None and b is not None and a <= s["t"] <= b]


def stats(samples, a, b, footprints):
    w = window(samples, a, b)
    fps = window([{"t": f["t"], **f} for f in footprints], a, b)
    if len(w) < 2:
        return None
    span = w[-1]["t"] - w[0]["t"]
    return {
        "dur_s": round(span),
        "app_rss_end": round(w[-1]["app_rss_mb"]),
        "app_rss_max": round(max(s["app_rss_mb"] for s in w)),
        "web_rss_end": round(w[-1]["web_rss_mb"]),
        "web_rss_max": round(max(s["web_rss_mb"] for s in w)),
        "app_cpu_pct": round(100 * (w[-1]["app_cpu_s"] - w[0]["app_cpu_s"]) / span),
        "web_cpu_pct": round(100 * (w[-1]["web_cpu_s"] - w[0]["web_cpu_s"]) / span),
        "fp": [(f["app_mb"], f["web_mb"]) for f in fps][-1] if fps else None,
        "analysed_delta": (w[-1]["analysed"] - w[0]["analysed"]) if w[0]["analysed"] is not None and w[-1]["analysed"] is not None else None,
    }


def phases(run):
    marks = [{"mark": m["mark"], "wall_ms": m["wall_ms"]} for m in run["marks"]]
    inter = mark_t(marks, "interactive")
    done = run["probe_done"]
    end = run["samples"][-1]["t"]
    out = {}
    if run["kind"] == "scroll":
        sc0, sc1 = mark_t(marks, "scroll_idle_start"), mark_t(marks, "scroll_idle")
        out["open idle 30 s"] = (inter, sc0)
        out["scripted scroll"] = (sc0, sc1)
        out["analysis"] = (sc1, done)
        out["idle after analysis"] = (done + 10 if done else None, end)
    elif run["kind"] == "expand":
        ex = mark_t(marks, "first_expand")
        out["open idle 30 s"] = (inter, inter + 30 if inter else None)
        out["expand + load"] = (inter + 30 if inter else None, ex)
        out["expanded idle"] = (ex + 5 if ex else None, end)
    elif run["kind"] == "backend-idle":
        opened = mark_t(marks, "load_state_encoded")
        out["idle first 30 s after load"] = (opened, done)
        out["idle after"] = (done + 10 if done else None, end)
    elif run["kind"] == "backend-analysis":
        out["launch + analysis of the 50"] = (run["start_wall"], done)
        out["idle after analysis"] = (done + 10 if done else None, end)
    else:
        s0 = mark_t(marks, "soak_start")
        out["open idle 30 s"] = (inter, s0)
        out["soak loop"] = (s0, done)
        out["idle after soak"] = (done + 10 if done else None, end)
    return {k: stats(run["samples"], a, b, run["footprints"]) for k, (a, b) in out.items()}


def summary(args):
    root = Path(args.root).resolve()
    lines = []
    for path in sorted((root / "results").glob("*/run.json")):
        run = json.loads(path.read_text())
        lines.append(f"### {run['name']} ({run['kind']}), protected paths unchanged: {run['protected_unchanged']}, screen locked: {run.get('screen_locked')}")
        lines.append("| phase | s | app RSS end/max MB | web RSS end/max MB | app CPU % | web CPU % | analysed | footprint app/web MB |")
        lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
        for name, st in phases(run).items():
            if st is None:
                lines.append(f"| {name} | - | - | - | - | - | - | - |")
                continue
            lines.append(f"| {name} | {st['dur_s']} | {st['app_rss_end']} / {st['app_rss_max']} | {st['web_rss_end']} / {st['web_rss_max']} | {st['app_cpu_pct']} | {st['web_cpu_pct']} | {st['analysed_delta']} | {'-' if not st['fp'] else '%.0f / %.0f' % (st['fp'][0] or 0, st['fp'][1])} |")
        lines.append("")
    text = "\n".join(lines)
    (root / "results/summary.md").write_text(text)
    print(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    sub = parser.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("name")
    r.add_argument("kind", choices=["scroll", "expand", "soak", "backend-idle", "backend-analysis"])
    r.add_argument("--unanalysed", type=int, default=50)
    r.add_argument("--idle-after", type=float, default=120)
    r.add_argument("--analysis-ms", type=int, default=60000)
    r.add_argument("--scroll-seconds", type=int, default=45)
    r.add_argument("--soak-minutes", type=float, default=25)
    r.add_argument("--soak-actions", default="")
    r.add_argument("--soak-steps", default="")
    r.add_argument("--footprint-secs", type=float, default=30)
    r.add_argument("--pressure-after", type=float, default=0)
    r.add_argument("--max-seconds", type=float, default=3600)
    sub.add_parser("summary")
    args = parser.parse_args()
    run(args) if args.cmd == "run" else summary(args)


if __name__ == "__main__":
    main()
