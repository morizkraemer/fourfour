"""Write a dev-path copy of a corpus manifest, keeping only tracks whose file exists
and whose content fingerprint matches the manifest."""

import argparse
import hashlib
import json
from pathlib import Path

CHUNK = 64 * 1024


def fingerprint(path: Path) -> str:
    size = path.stat().st_size
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read(CHUNK))
        if size > CHUNK:
            f.seek(max(0, size - CHUNK))
            h.update(f.read(CHUNK))
    h.update(str(size).encode())
    return h.hexdigest()


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("manifest")
    p.add_argument("out")
    p.add_argument("--strip-prefix", required=True)
    p.add_argument("--dataset-root", required=True)
    p.add_argument("--rename-map", help="TSV of '<path under dataset-root>\\t<relative path in manifest>' for files that moved")
    args = p.parse_args()

    manifest = json.loads(Path(args.manifest).read_text())
    root = Path(args.dataset_root)
    moved = {}
    if args.rename_map:
        for line in Path(args.rename_map).read_text().splitlines():
            src, rel = line.split("\t")
            moved[rel] = src
    kept, missing, mismatch = [], [], []
    for entry in manifest["entries"]:
        if not entry["path"].startswith(args.strip_prefix):
            raise SystemExit(f"path outside prefix: {entry['path']}")
        rel = entry["path"][len(args.strip_prefix):]
        local = root / moved.get(rel, rel)
        if not local.is_file():
            missing.append(entry["id"])
            continue
        if fingerprint(local) != entry["content_fingerprint"]:
            mismatch.append(entry["id"])
            continue
        kept.append({**entry, "path": str(local)})

    out = {k: v for k, v in manifest.items() if k not in ("entries", "source_directory", "baseline_path")}
    out.update(
        name=manifest["name"] + "-dev",
        num_entries=len(kept),
        num_bpm_ground_truth=sum(1 for e in kept if (e.get("ground_truth") or {}).get("bpm") is not None),
        num_key_ground_truth=sum(1 for e in kept if (e.get("ground_truth") or {}).get("key") is not None),
        dev_filter={"manifest_entries": len(manifest["entries"]), "missing": len(missing), "fingerprint_mismatch": len(mismatch)},
        entries=kept,
    )
    Path(args.out).write_text(json.dumps(out, indent=1))
    print(f"{args.manifest}: {len(manifest['entries'])} entries, kept {len(kept)}, missing {len(missing)}, fingerprint mismatch {len(mismatch)}")


if __name__ == "__main__":
    main()
