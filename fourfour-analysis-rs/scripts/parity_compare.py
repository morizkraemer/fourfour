"""Compare stored Python and Rust analyzer outputs field by field. usage: parity_compare.py <out_dir>"""
import json
import sys
from pathlib import Path

import numpy as np

out = Path(sys.argv[1])
rust = json.loads((out / "rust.json").read_text())
timings = json.loads((out / "py_timings.json").read_text())["tracks"]


def camelot_rel(a, b):
    if not a or not b:
        return "missing"
    if a == b:
        return "exact"
    na, la = int(a[:-1]), a[-1]
    nb, lb = int(b[:-1]), b[-1]
    if na == nb:
        return "relative"
    if la == lb and (na - nb) % 12 in (1, 11):
        return "fifth"
    return "other"


def corr(a, b):
    n = min(len(a), len(b))
    a, b = np.asarray(a[:n], float), np.asarray(b[:n], float)
    if a.std() == 0 or b.std() == 0:
        return float("nan")
    return float(np.corrcoef(a, b)[0, 1])


rows = []
for i, entry in enumerate(rust):
    py = json.loads((out / f"py_{i}.json").read_text())["deeprhythm_essentia"]
    r = entry["result"]
    name = Path(entry["path"]).name[:28]
    py_beats = np.array([b["time_seconds"] for b in py["beats"]])
    rs_beats = np.array([b["time_ms"] / 1000 for b in r["beat_grid"]["beats"]])
    nearest = np.abs(rs_beats[:, None] - py_beats[None, :]).min(axis=1) if len(py_beats) and len(rs_beats) else np.array([np.nan])
    period = 60 / py["bpm"] if py["bpm"] else 0.5
    offset = ((rs_beats[0] - py_beats[0] + period / 2) % period) - period / 2 if len(py_beats) and len(rs_beats) else float("nan")
    py_prev = np.array(py["waveform_preview"])
    rs_prev = np.array(r["waveform"]["data"])
    py_h, rs_h = py_prev & 31, rs_prev & 31
    py_col = np.array([[c["r"], c["g"], c["b"]] for c in py["waveform_colors"]])
    rs_col = np.array(r["color_waveform"]["detail"])
    n = min(len(py_col), len(rs_col))
    py_pos = [b["bar_position"] for b in py["beats"]]
    rs_pos = [b["bar_position"] for b in r["beat_grid"]["beats"]]
    rows.append({
        "track": name,
        "py_bpm": py["bpm"], "rs_bpm": round(r["bpm"], 2),
        "bpm_ratio": round(r["bpm"] / py["bpm"], 3) if py["bpm"] else None,
        "py_key": py["key"], "rs_key": r["key"], "key_rel": camelot_rel(py["key"], r["key"]),
        "beats": (len(py_beats), len(rs_beats)),
        "phase_offset_ms": round(offset * 1000, 1),
        "pwav_height_mad": round(float(np.abs(py_h.astype(int) - rs_h.astype(int)).mean()), 2),
        "pwav_height_corr": round(corr(py_h, rs_h), 3),
        "pwav_white_mad": round(float(np.abs((py_prev >> 5).astype(int) - (rs_prev >> 5).astype(int)).mean()), 2),
        "cols": (len(py_col), len(rs_col)),
        "col_mad_lmh": [round(float(np.abs(py_col[:n, c] - rs_col[:n, c]).mean()), 1) for c in range(3)],
        "col_corr_lmh": [round(corr(py_col[:n, c], rs_col[:n, c]), 3) for c in range(3)],
        "cues": (len(py["cue_points"]), len(r["cue_points"])),
        "energy": (py.get("energy"), r.get("energy")),
        "py_secs": round(timings[i]["secs"], 1),
    })
for row in rows:
    print(json.dumps(row))
(out / "parity.json").write_text(json.dumps(rows, indent=1))
