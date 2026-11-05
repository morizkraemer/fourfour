#!/usr/bin/env python3
"""Mac night 3 summaries. Each item subcommand writes <out>.md and <out>.status ("PASS|FAIL|SKIP<TAB>one line").

usage:
  night3-summary.py item1 <app-run.jsonl> <out>
  night3-summary.py item2 <perf-dir> <out>
  night3-summary.py item3 <first.jsonl> <out>
  night3-summary.py item4 <sweep.tsv> <out>
  night3-summary.py item5 <sweep.tsv> <out>
  night3-summary.py assemble <night-dir>
"""
import glob
import json
import os
import re
import statistics
import sys

LAST_MAC = {"cold_first_row": 868, "cold_interactive": 910, "warm_first_row": 628, "warm_interactive": 645, "first_expand_longest": 34, "scroll_p95": 18}
LAST_MAC_SOURCE = "R89, 2026-11-03, ov-1028 c2bd463"
REGRESSION_FACTOR = 1.15
EXPAND_TRIGGER_MS = 50
SCROLL_P95_LIMIT_MS = 25
THROUGHPUT_FLOOR = 0.95


def med(values):
    values = [v for v in values if isinstance(v, (int, float))]
    return statistics.median(values) if values else None


def fmt(value, digits=0):
    return "-" if value is None else f"{value:.{digits}f}"


def write(out, status, line, md):
    with open(out + ".md", "w") as f:
        f.write(md.rstrip() + "\n")
    with open(out + ".status", "w") as f:
        f.write(f"{status}\t{line}\n")


def marks_of(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def run_numbers(path):
    marks = marks_of(path)

    def by(name):
        return next((m for m in marks if m["mark"] == name), None)

    first, inter, start, built = by("first_row"), by("interactive"), by("load_state_start"), by("load_state_built")
    done, expand, scroll = by("probe_done"), by("first_expand"), by("scroll_during_analysis")
    return {
        "first_row": first["ms"] if first else None,
        "interactive": first["ms"] + (inter["values"]["at_page_ms"] - first["values"]["page_ms"]) if first and inter else None,
        "load_state": built["ms"] - start["ms"] if built and start else None,
        "done": bool(done),
        "error": (done or {}).get("values", {}).get("error"),
        "expand_longest": expand["values"].get("longest_frame_ms") if expand else None,
        "scroll_p95": scroll["values"].get("frame_p95_ms") if scroll else None,
        "cycles": [m["values"]["cycle_ms"] for m in marks if m["mark"] == "soak_cycle"],
    }


def item1(jsonl, out):
    if not os.path.exists(jsonl):
        return write(out, "FAIL", "app did not write a probe log", "No probe log: the app did not start or the build is missing.")
    r = run_numbers(jsonl)
    cycles = r["cycles"]
    first_cycle, later = (cycles[0], med(cycles[1:])) if cycles else (None, None)
    problems = []
    if r["first_row"] is None:
        problems.append("no first row")
    if r["interactive"] is None:
        problems.append("never interactive")
    if r["error"]:
        problems.append(r["error"])
    if r["first_row"] and r["first_row"] > LAST_MAC["cold_first_row"] * REGRESSION_FACTOR:
        problems.append(f"first row {fmt(r['first_row'])} ms is over {int((REGRESSION_FACTOR - 1) * 100)}% slower than {LAST_MAC['cold_first_row']} ms")
    if first_cycle is None:
        problems.append("no settings cycle marks")
    md = f"""## 1. ov-1028 build, scratch-HOME launch, first Settings open

Fresh HOME with a copy of the 10k fixture library (first launch, so compare with the cold column). Last Mac numbers: {LAST_MAC_SOURCE}.

| metric | now | last Mac | change |
|---|---:|---:|---:|
| process start to first row (rows on screen), ms | {fmt(r['first_row'])} | {LAST_MAC['cold_first_row']} | {fmt((r['first_row'] or 0) - LAST_MAC['cold_first_row'])} |
| process start to interactive, ms | {fmt(r['interactive'])} | {LAST_MAC['cold_interactive']} | {fmt((r['interactive'] or 0) - LAST_MAC['cold_interactive'])} |
| app load_state, ms | {fmt(r['load_state'])} | 53-67 | |
| first Settings open (soak cycle 1), ms | {fmt(first_cycle)} | - | |
| later Settings cycles, median ms | {fmt(later)} | - | |
| first open minus later (lazy dialog mount, includes one-off work) | {fmt(first_cycle - later) if first_cycle is not None and later is not None else '-'} | - | |

A soak cycle here is the whole Settings action (open, click each nav item, close), so the numbers are a proxy: the probe has no dedicated mark for the first dialog open. Read the difference between cycle 1 and the later cycles, not the absolute value.
"""
    status = "FAIL" if problems else "PASS"
    write(out, status, "; ".join(problems) if problems else f"first row {fmt(r['first_row'])} ms, interactive {fmt(r['interactive'])} ms, settings first {fmt(first_cycle)} ms vs later {fmt(later)} ms", md)


def item2(perf_dir, out):
    files = sorted(glob.glob(os.path.join(perf_dir, "app-run-*.jsonl")))
    if not files:
        return write(out, "FAIL", "perf-night wrote no app runs", "perf-night.sh produced no app-run-*.jsonl.")
    runs = {os.path.basename(f)[len("app-run-"):-len(".jsonl")]: run_numbers(f) for f in files}
    warm = [r for k, r in runs.items() if re.fullmatch(r"\d+", k) and k != "1"]
    expand = [r["expand_longest"] for k, r in runs.items() if k.startswith("expand-")]
    cold = runs.get("1", {})
    scroll = runs.get("analysis", {}).get("scroll_p95")
    warm_first, warm_inter = med([r["first_row"] for r in warm]), med([r["interactive"] for r in warm])
    expand_med = med(expand)
    unfinished = [k for k, r in runs.items() if not r["done"] or r["error"]]
    problems = []
    if unfinished:
        problems.append("runs without probe_done or with an error: " + ", ".join(unfinished))
    for name, now, last in [("warm first row", warm_first, LAST_MAC["warm_first_row"]), ("warm interactive", warm_inter, LAST_MAC["warm_interactive"]), ("cold first row", cold.get("first_row"), LAST_MAC["cold_first_row"])]:
        if now is None:
            problems.append(f"{name} missing")
        elif now > last * REGRESSION_FACTOR:
            problems.append(f"{name} {fmt(now)} ms vs {last} ms")
    if expand_med is not None and expand_med > EXPAND_TRIGGER_MS:
        problems.append(f"first expand longest frame {fmt(expand_med)} ms over {EXPAND_TRIGGER_MS}")
    if scroll is not None and scroll > SCROLL_P95_LIMIT_MS:
        problems.append(f"scroll during analysis p95 {fmt(scroll)} ms over {SCROLL_P95_LIMIT_MS}")
    headline = ""
    summary = os.path.join(perf_dir, "summary.md")
    if os.path.exists(summary):
        with open(summary) as f:
            headline = next((line.strip() for line in f if "against targets" in line), "")
    md = f"""## 2. perf-night 10k, 3 runs, 5 expand runs (ov-1028)

Gate: no run more than {int((REGRESSION_FACTOR - 1) * 100)}% slower than the last Mac night ({LAST_MAC_SOURCE}), first expand under the {EXPAND_TRIGGER_MS} ms staged-reveal trigger, scroll during analysis p95 under {SCROLL_P95_LIMIT_MS} ms. The 10k targets in `scripts/perf-budgets.json` are reported by perf-night itself ({headline or 'no target line'}); they are not the gate here.

| metric | now | last Mac |
|---|---:|---:|
| warm first row (median of runs 2-3), ms | {fmt(warm_first)} | {LAST_MAC['warm_first_row']} |
| warm interactive, ms | {fmt(warm_inter)} | {LAST_MAC['warm_interactive']} |
| cold first row, ms | {fmt(cold.get('first_row'))} | {LAST_MAC['cold_first_row']} |
| cold interactive, ms | {fmt(cold.get('interactive'))} | {LAST_MAC['cold_interactive']} |
| app load_state (median over runs), ms | {fmt(med([r['load_state'] for r in runs.values()]))} | 53-67 |
| first expand, longest frame (median of {len(expand)}), ms | {fmt(expand_med)} | {LAST_MAC['first_expand_longest']} |
| scroll during analysis, frame p95, ms | {fmt(scroll)} | {LAST_MAC['scroll_p95']} |

Full table with every run, the UI headroom rows and the backend suite: `perf/summary.md`.
"""
    write(out, "FAIL" if problems else "PASS", "; ".join(problems) if problems else f"warm first row {fmt(warm_first)} ms, interactive {fmt(warm_inter)} ms, expand {fmt(expand_med)} ms", md)


def item3(path, out):
    if not os.path.exists(path):
        return write(out, "FAIL", "no first-result trials", "night3-first.py wrote nothing.")
    rows = [json.loads(line) for line in open(path) if line.strip()]
    labels = sorted({r["label"] for r in rows}, key=lambda s: (s != "ov-1028", s))
    stats = {}
    for label in labels:
        mine = [r for r in rows if r["label"] == label]
        times = [r["first_s"] for r in mine if r["first_s"] is not None]
        stats[label] = {"n": len(mine), "ok": len(times), "median": med(times), "min": min(times) if times else None, "max": max(times) if times else None, "quiet": sum(1 for r in mine if r["quiet"]), "load": med([r["load_before"] for r in mine])}
    base, new = stats.get("ov-1028"), stats.get("r110-first-result")
    lines = ["| build | trials | quiet start | load before (median) | first result median s | min | max |", "|---|---:|---:|---:|---:|---:|---:|"]
    for label, s in stats.items():
        lines.append(f"| {label} | {s['ok']}/{s['n']} | {s['quiet']}/{s['n']} | {fmt(s['load'], 1)} | {fmt(s['median'], 1)} | {fmt(s['min'], 1)} | {fmt(s['max'], 1)} |")
    problems = []
    if not base or not new or not base["median"] or not new["median"]:
        problems.append("a build has no finished trial")
    else:
        quiet = min(base["quiet"], new["quiet"])
        if quiet < 3:
            problems.append(f"machine not quiet: only {quiet} quiet trials per build, the result says little")
        if new["median"] >= base["median"]:
            problems.append(f"r110 {fmt(new['median'], 1)} s is not faster than ov-1028 {fmt(base['median'], 1)} s")
    md = f"""## 3. r110 first result, quiet machine (rten, 24 full-length masters)

Interleaved trials, a trial starts only when the machine is at least 80% idle (waits up to 60 s, then starts anyway and counts as not quiet). rten only: the r110 burst changes nothing with CoreML (R110). Linux box result for comparison: 20.3 s base, 21.7 s r110 under 6-13 cores of other work; a quiet machine was predicted at about 16 s base and 9-10 s r110.

""" + "\n".join(lines)
    write(out, "FAIL" if problems else "PASS", "; ".join(problems) if problems else f"r110 {fmt(new['median'], 1)} s vs ov-1028 {fmt(base['median'], 1)} s", md)


def read_sweep(tsv):
    rows = []
    with open(tsv) as f:
        header = f.readline().rstrip("\n").split("\t")
        for line in f:
            row = dict(zip(header, line.rstrip("\n").split("\t")))
            rows.append(row)
    return rows


def number(row, key):
    try:
        return float(row[key])
    except (KeyError, ValueError):
        return None


def grouped(rows):
    labels = list(dict.fromkeys(r["label"] for r in rows))
    out = {}
    for label in labels:
        mine = [r for r in rows if r["label"] == label]
        out[label] = {
            "n": len(mine),
            "bad": sum(1 for r in mine if r["exit"] != "0" or r["errors"] not in ("0", "")),
            "tpm": med([number(r, "tracks_per_min") for r in mine]),
            "first": med([number(r, "first_result_s") for r in mine]),
            "rss": med([number(r, "peak_rss_mb") for r in mine]),
            "cpu": med([number(r, "cpu_s") for r in mine]),
        }
    return out


def table(g, reference):
    lines = ["| config | runs | tracks/min | vs " + reference + " | first result s | peak RSS MB | CPU s | bad runs |", "|---|---:|---:|---:|---:|---:|---:|---:|"]
    ref = g.get(reference)
    for label, s in g.items():
        rel = f"{s['tpm'] / ref['tpm'] * 100:.0f}%" if ref and ref["tpm"] and s["tpm"] else "-"
        lines.append(f"| {label} | {s['n']} | {fmt(s['tpm'], 1)} | {rel} | {fmt(s['first'], 1)} | {fmt(s['rss'])} | {fmt(s['cpu'])} | {s['bad']} |")
    return "\n".join(lines)


def item4(tsv, out):
    if not os.path.exists(tsv):
        return write(out, "FAIL", "no sweep data", "No sweep.tsv.")
    g = grouped(read_sweep(tsv))
    ref = "a-coreml-1500"
    a, b, c = g.get(ref), g.get("b-coreml-500"), g.get("c-int8-rten-500")
    problems = []
    if not (a and b and c) or not a["tpm"]:
        problems.append("a config has no finished run")
    else:
        for name, s in [("chunk 500 CoreML (43)", b), ("int8 rten 500 (44)", c)]:
            if s["bad"]:
                problems.append(f"{name}: {s['bad']} bad runs")
            if s["tpm"] and s["tpm"] < a["tpm"] * THROUGHPUT_FLOOR:
                problems.append(f"{name}: {s['tpm'] / a['tpm'] * 100:.0f}% of CoreML 1500, floor {int(THROUGHPUT_FLOOR * 100)}%")
            if s["rss"] and a["rss"] and s["rss"] > a["rss"]:
                problems.append(f"{name}: RSS {fmt(s['rss'])} MB above {fmt(a['rss'])}")
    md = f"""## 4. Chunk 500 on CoreML (Waiting 43) and int8 at chunk 500 (Waiting 44) against CoreML 1500

Binary `r98-mac-pair` (has `FOURFOUR_BEAT_CHUNK` and `FOURFOUR_BEAT_MODEL`; ov-1028 has neither), `bench_analysis` built with coreml, 200 fixture tracks (the long masters first), `nice -n 10`, interleaved rounds, medians. Row d is the ov-1028 binary at its defaults, to show the base drift. Gate (r98): tracks/min at least {int(THROUGHPUT_FLOOR * 100)}% of CoreML 1500 and peak RSS not higher. CPU is user+sys from `time -l` and leaves out the Neural Engine and GPU.

""" + table(g, ref) + """

Not measured here: beat and BPM quality. The r96/r98 scores (test grid 171 = 171, dev 44 to 43, int8 within 1 track) stand; a CoreML at chunk 500 score needs the o3 reference set, which is not on the Mac.
"""
    write(out, "FAIL" if problems else "PASS", "; ".join(problems) if problems else f"500 CoreML {fmt(b['tpm'], 1)}, int8 500 {fmt(c['tpm'], 1)} vs CoreML 1500 {fmt(a['tpm'], 1)} trk/min", md)


def item5(tsv, out):
    if not os.path.exists(tsv):
        return write(out, "FAIL", "no sweep data", "No sweep.tsv.")
    g = grouped(read_sweep(tsv))
    one, two = g.get("cores-1"), g.get("cores-2")
    problems = []
    if not one or not two or not one["tpm"] or not two["tpm"]:
        problems.append("a config has no finished run")
    elif one["bad"] or two["bad"]:
        problems.append("bad runs")
    gain = (one["tpm"] / two["tpm"] - 1) * 100 if not problems else None
    md = f"""## 5. Analysis throughput with the UI busy, cores-1 against cores-2 (Waiting 29)

`bench_analysis` (ov-1028, coreml build), 24 long masters, `--ui-busy 2000/6000` (the main thread busy one third of the time, which holds the analysis as the app does), interleaved rounds, medians. cores-2 is the shipped default. The box prediction (r108): +13% on 16 cores, +24% on 8.

""" + table(g, "cores-2") + """

Judge the gain against what scrolling feels like: perf-night's analysis rows (item 2) show frame p95 with cores-1 (`analysis-before`) and with the default.
"""
    write(out, "FAIL" if problems else "PASS", "; ".join(problems) if problems else f"cores-1 {fmt(one['tpm'], 1)} vs cores-2 {fmt(two['tpm'], 1)} trk/min ({gain:+.0f}%)", md)


def assemble(night):
    results = os.path.join(night, "results")
    rows, body = [], []
    for n in range(1, 6):
        status_file = os.path.join(results, f"item{n}.status")
        status, line = ("SKIP", "not run")
        if os.path.exists(status_file):
            status, line = open(status_file).read().rstrip("\n").split("\t", 1)
        rows.append(f"| {n} | {status} | {line} |")
        md_file = os.path.join(results, f"item{n}.md")
        if os.path.exists(md_file):
            body.append(open(md_file).read())
    meta = open(os.path.join(results, "meta.txt")).read() if os.path.exists(os.path.join(results, "meta.txt")) else ""
    protected = open(os.path.join(results, "protected.txt")).read().strip() if os.path.exists(os.path.join(results, "protected.txt")) else "not checked"
    head = ["# Mac night 3", "", meta.rstrip(), "", f"Protected paths (Pioneer, rekordbox, the real app library): {protected}", "", "| item | result | numbers |", "|---|---|---|"] + rows
    head += ["", "Item 6 (ears and eyes) is a checklist for Merlin, not automated: `docs/architecture/perf-run-2026-11.md`, section Mac night 3.", ""]
    with open(os.path.join(night, "summary.md"), "w") as f:
        f.write("\n".join(head) + "\n" + "\n".join(body))
    print("\n".join(head))


COMMANDS = {"item1": item1, "item2": item2, "item3": item3, "item4": item4, "item5": item5}

if __name__ == "__main__":
    if sys.argv[1] == "assemble":
        assemble(sys.argv[2])
    else:
        COMMANDS[sys.argv[1]](sys.argv[2], sys.argv[3])
