import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

here = Path(__file__).resolve().parent
repo = here.parents[1]
manifest = json.load(open(repo / "benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json"))
audio = Path(os.environ.get("AUDIO", "/home/merlin/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio"))
step = int(sys.argv[1]) if len(sys.argv) > 1 else 5
workers = int(sys.argv[2]) if len(sys.argv) > 2 else 6

entries = manifest["entries"][::step]
(here / "results").mkdir(exist_ok=True)


def run(entry):
    name = entry["path"].rsplit("/", 1)[-1]
    path = audio / name
    if not path.exists():
        return None
    out = subprocess.run([str(here / "target/release/ffproto"), "--no-beats", str(path)], capture_output=True, text=True)
    if out.returncode != 0:
        return {"id": entry["id"], "error": out.stderr[-200:]}
    r = json.loads(out.stdout)
    return {
        "id": entry["id"],
        "truth": entry["ground_truth"]["key"],
        "camelot": r["stratum"]["camelot"],
        "bpm": r["stratum"]["bpm"],
        "key_confidence": r["stratum"]["key_confidence"],
        "stratum_ms": r["ms"]["stratum"],
    }


with ThreadPoolExecutor(workers) as ex:
    rows = [r for r in ex.map(run, entries) if r]

json.dump(rows, open(here / "results/key-subset.json", "w"))


def parse(c):
    return int(c[:-1]), c[-1]


exact = rel = nb = err = 0
for r in rows:
    if "error" in r:
        err += 1
        continue
    n1, l1 = parse(r["camelot"])
    n2, l2 = parse(r["truth"])
    if (n1, l1) == (n2, l2):
        exact += 1
    elif n1 == n2:
        rel += 1
    elif l1 == l2 and (n1 - n2) % 12 in (1, 11):
        nb += 1
n = len(rows) - err
print(f"tracks={n} errors={err} exact={exact} ({exact / n:.1%}) relative={rel} neighbour={nb} "
      f"exact+rel+nb={(exact + rel + nb) / n:.1%} mean_stratum_ms={sum(r['stratum_ms'] for r in rows if 'error' not in r) / n:.0f}")
