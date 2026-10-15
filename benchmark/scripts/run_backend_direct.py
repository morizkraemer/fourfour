"""Run a registered backend over a corpus without the result cache and write raw records.

Output layout matches fourfour-benchmark: <results>/<run_id>/raw/<variant>.json.
Needed because the benchmark runner's cache cannot serialise the bytes waveform_preview field.
"""

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

from fourfour_analysis.backends.registry import load_backend
from fourfour_analysis.config import Settings
from fourfour_analysis.manifest import load_corpus


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--corpus", required=True)
    p.add_argument("--variant", default="deeprhythm_essentia")
    p.add_argument("--features", default="bpm,key")
    p.add_argument("--limit", type=int)
    p.add_argument("--run-id")
    p.add_argument("--shard", default="0/1", help="i/n: analyse entries with index %% n == i")
    args = p.parse_args()

    settings = Settings.from_cwd()
    features = set(args.features.split(","))
    shard_i, shard_n = map(int, args.shard.split("/"))
    entries = load_corpus(args.corpus)[: args.limit][shard_i::shard_n]
    backend = load_backend(args.variant, settings, features=features)

    run_id = args.run_id or f"run-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    raw_dir = settings.results_dir / run_id / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)

    records = []
    start = time.monotonic()
    for i, entry in enumerate(entries, 1):
        t0 = time.monotonic()
        try:
            res = backend.analyze_track(entry.path)
            records.append({
                "track_id": entry.id, "status": "ok",
                "result": {"bpm": res.bpm, "key": res.key, "energy": res.energy,
                           "elapsed_seconds": time.monotonic() - t0},
            })
        except Exception as e:
            records.append({"track_id": entry.id, "status": "failed", "result": None, "error": f"{type(e).__name__}: {e}"})
        if i % 10 == 0 or i == len(entries):
            print(f"[{i}/{len(entries)}] {time.monotonic() - start:.0f}s", file=sys.stderr, flush=True)

    (raw_dir / f"{args.variant}.shard{shard_i}of{shard_n}.json").write_text(json.dumps(records))
    (settings.results_dir / run_id / f"meta.shard{shard_i}.json").write_text(json.dumps({
        "run_id": run_id, "corpus": Path(args.corpus).name, "variant": args.variant,
        "features": sorted(features), "tracks": len(entries), "wall_seconds": time.monotonic() - start,
    }))
    print(run_id)


if __name__ == "__main__":
    main()
