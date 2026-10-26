"""Run the Python analyzer and the Rust analyzer on the same tracks and store raw outputs.

usage: parity_run.py <paths.txt> <out_dir> [python_parallel] [threads_per_python]
"""
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

root = Path(__file__).resolve().parents[2]
paths = [p for p in Path(sys.argv[1]).read_text().splitlines() if p.strip()]
out = Path(sys.argv[2])
parallel = int(sys.argv[3]) if len(sys.argv) > 3 else 2
threads = sys.argv[4] if len(sys.argv) > 4 else "3"
out.mkdir(parents=True, exist_ok=True)
python = root / "analysis/.venv/bin/python"
env = dict(os.environ, OMP_NUM_THREADS=threads, MKL_NUM_THREADS=threads)


def run_python(item):
    i, path = item
    t = time.time()
    proc = subprocess.run(
        [str(python), "-m", "fourfour_analysis", "analyze", path, "--backend", "deeprhythm_essentia", "--json"],
        capture_output=True, text=True, env=env, timeout=600,
    )
    secs = time.time() - t
    (out / f"py_{i}.json").write_text(proc.stdout)
    return {"i": i, "path": path, "secs": secs, "rc": proc.returncode}


t0 = time.time()
with ThreadPoolExecutor(parallel) as pool:
    timings = list(pool.map(run_python, enumerate(paths)))
(out / "py_timings.json").write_text(json.dumps({"wall": time.time() - t0, "tracks": timings}))

t = time.time()
binary = root / "target/release/analyze-json"
rust = subprocess.run([str(binary), "--threads", "6", *paths], capture_output=True, text=True)
(out / "rust.json").write_text(rust.stdout)
(out / "rust_timings.json").write_text(json.dumps({"wall": time.time() - t, "rc": rust.returncode}))
print("python wall", time.time() - t0 - (time.time() - t), "rust wall", time.time() - t)
