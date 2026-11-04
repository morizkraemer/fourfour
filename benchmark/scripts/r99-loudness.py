#!/usr/bin/env python3
"""Compare fourfour's integrated loudness against ffmpeg's ebur128 filter on real tracks.

usage: r99-loudness.py <datasets-dir> <scratch-dir> <loudness_report binary>

Picks 21 tracks (7 Beatport mp3, 5 library mp3, 4 flac, 3 wav 44.1 kHz, 1 wav 48 kHz, fixed seed, 1-10 min), adds a
mono and a 48 kHz derivative of the first one, runs both meters, prints a table and summary. ffmpeg is a reference only.
"""
import glob
import json
import math
import os
import random
import re
import statistics
import subprocess
import sys

datasets, scratch, binary = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(scratch, exist_ok=True)
random.seed(99)


def probe(path):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
         "stream=sample_rate,channels,codec_name:format=duration", "-of", "json", path],
        capture_output=True, text=True).stdout
    try:
        j = json.loads(out)
        s = j["streams"][0]
        s["duration"] = float(j["format"]["duration"])
        return s
    except Exception:
        return None


def select():
    beatport = sorted(glob.glob(os.path.join(datasets, "beatport-edm-key/audio/*.mp3")))
    library = []
    for root in ["rekordbox-testdata-2025", "rekordbox-library/share", "rekordbox-library-2026-10-25/share"]:
        for r, _, files in os.walk(os.path.join(datasets, root)):
            library += [os.path.join(r, f) for f in files if f.lower().endswith((".mp3", ".flac", ".wav"))]
    library = sorted(set(library))
    random.shuffle(library)
    random.shuffle(beatport)
    want = {"mp3": 5, "flac": 4, "pcm48": 1, "pcm44": 3}
    got = {k: [] for k in want}
    for p in library:
        s = probe(p)
        if not s or not 60 <= s["duration"] <= 600 or s["channels"] != 2:
            continue
        c = s["codec_name"]
        k = "mp3" if c == "mp3" else "flac" if c == "flac" else ("pcm48" if s["sample_rate"] == "48000" else "pcm44") if c.startswith("pcm") else None
        if k and len(got[k]) < want[k]:
            got[k].append(p)
    chosen = []
    for p in beatport:
        s = probe(p)
        if s and 60 <= s["duration"] <= 600:
            chosen.append(p)
        if len(chosen) == 7:
            break
    chosen += sum(got.values(), [])
    mono, rate48 = os.path.join(scratch, "mono.flac"), os.path.join(scratch, "rate48.flac")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", chosen[0], "-ac", "1", mono], check=True)
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", chosen[0], "-ar", "48000", rate48], check=True)
    return chosen + [mono, rate48]


def ffmpeg_ebur128(path):
    r = subprocess.run(["ffmpeg", "-nostats", "-v", "error", "-i", path, "-af",
                        "ebur128=metadata=1:peak=true,ametadata=mode=print:file=-", "-f", "null", "-"],
                       capture_output=True, text=True)
    integrated = [float(m) for m in re.findall(r"lavfi\.r128\.I=(-?[\d.]+)", r.stdout)]
    peaks = [float(m) for m in re.findall(r"lavfi\.r128\.true_peaks_ch\d+=([\d.]+)", r.stdout)]
    return integrated[-1], 20 * math.log10(max(peaks))


tracks_file = os.path.join(scratch, "tracks.txt")
if os.path.exists(tracks_file):
    tracks = open(tracks_file).read().split("\n")[:-1]
else:
    tracks = select()
    open(tracks_file, "w").write("\n".join(tracks) + "\n")

ours = {}
for line in subprocess.run([binary, "--list", tracks_file], capture_output=True, text=True, check=True).stdout.splitlines():
    d = json.loads(line)
    ours[d["path"]] = d

rows = []
print(f"{'track':44} {'fmt':>10} {'ref LUFS':>9} {'ours':>9} {'dLU':>6} {'ref TP':>7} {'ours TP':>8} {'dTP':>6} {'gain':>6} {'ms':>6}")
for p in tracks:
    s = probe(p)
    ref_l, ref_p = ffmpeg_ebur128(p)
    d = ours[p]
    name = os.path.basename(p)[:44]
    fmt = f"{s['codec_name'][:4]}/{int(s['sample_rate']) // 1000}k/{s['channels']}"
    rows.append((ref_l - d["lufs"], ref_p - d["peak_db"], d["loudness_ms"] / d["duration_s"]))
    print(f"{name:44} {fmt:>10} {ref_l:9.1f} {d['lufs']:9.2f} {d['lufs'] - ref_l:6.2f} {ref_p:7.1f} {d['peak_db']:8.2f} {d['peak_db'] - ref_p:6.2f} {d['gain_db']:6.2f} {d['loudness_ms']:6.0f}")

dl = [abs(r[0]) for r in rows]
dp = [abs(r[1]) for r in rows]
print(f"\ntracks {len(rows)}  |dLUFS| mean {statistics.mean(dl):.3f} max {max(dl):.3f}   |dTP| mean {statistics.mean(dp):.3f} max {max(dp):.3f}")
