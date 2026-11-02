import re
import sys


def parse_time(path):
    text = open(path).read()
    rss_mb = cpu_s = None
    m = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", text)
    if m:
        rss_mb = int(m.group(1)) / 1024
    m = re.search(r"^\s*(\d+)\s+maximum resident set size", text, re.M)
    if m:
        rss_mb = int(m.group(1)) / 1024 / 1024
    user = re.search(r"User time \(seconds\):\s*([\d.]+)", text) or re.search(r"([\d.]+)\s+user", text)
    system = re.search(r"System time \(seconds\):\s*([\d.]+)", text) or re.search(r"([\d.]+)\s+sys", text)
    if user and system:
        cpu_s = float(user.group(1)) + float(system.group(1))
    return rss_mb, cpu_s


def parse_bench(path):
    text = open(path).read()
    m = re.search(
        r"^BENCH .*?tracks=(\d+) errors=(\d+) wall=([\d.]+)s tracks_per_min=([\d.]+).*?peak_rss_tree=(\d+)MB",
        text,
        re.M,
    )
    return m.groups() if m else ("", "", "", "", "")


def fmt(value, digits):
    return "" if value is None else f"{value:.{digits}f}"


def row(budget, status, bench_out, time_out):
    tracks, errors, wall, per_min, tree = parse_bench(bench_out)
    rss_mb, cpu_s = parse_time(time_out)
    print("\t".join([budget, tracks, errors, wall, per_min, fmt(rss_mb, 0), tree, fmt(cpu_s, 1), status]))


def table(tsv):
    body = [line.rstrip("\n").split("\t") for line in open(tsv)][1:]
    base = next((r for r in body if r[0] == "unset" and r[4]), None)
    out = [
        "| budget MB | tracks | errors | wall s | tracks/min | vs unset | peak RSS MB | tree RSS MB | CPU s | exit |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for r in body:
        rel = f"{float(r[4]) / float(base[4]) * 100:.0f}%" if base and r[4] else ""
        out.append("| " + " | ".join([r[0], r[1], r[2], r[3], r[4], rel, r[5], r[6], r[7], r[8]]) + " |")
    print("\n".join(out))


if __name__ == "__main__":
    if sys.argv[1] == "row":
        row(*sys.argv[2:6])
    else:
        table(sys.argv[2])
