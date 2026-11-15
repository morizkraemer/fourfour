#!/usr/bin/env python3
"""Build the file lists and the genre-prior table for the energy evaluation (r100).

usage: energy_prep.py <out_dir>

Writes into <out_dir>:
  beatport.json      track path, Beatport genre, a priori genre energy, dev/test split
  beatport.txt       one path per line
  user_sample.txt    seeded random sample of the user's own library (full-length tracks)
  user_lossless.txt  seeded sample of lossless tracks for the re-encode test
  user_cost.txt      seeded sample for the CPU cost measurement
"""
import hashlib
import json
import os
import random
import re
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

DATASETS = Path(os.environ.get("BENCHMARK_DATASETS", Path(__file__).resolve().parents[1] / "datasets"))
BEATPORT = DATASETS / "beatport-edm-key"
LIBRARY = DATASETS / "rekordbox-testdata-2025"

GENRE_PRIOR = {
    "Electronica / Downtempo": 2,
    "Hip-Hop / R&B": 4,
    "Deep House": 4,
    "Minimal / Deep Tech": 5,
    "Minimal": 5,
    "Progressive House": 6,
    "House": 6,
    "Funky / Groove / Jackin' House": 6,
    "Tech House": 6,
    "Breaks": 6,
    "Techno": 7,
    "Trance": 7,
    "Electro House": 8,
    "Dubstep": 8,
    "Drum & Bass": 8,
    "Psy-Trance": 8,
    "Hard Dance": 9,
    "Big Room": 9,
}


def xlsx_genres(path):
    main = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
    with zipfile.ZipFile(path) as z:
        strings = ["".join(t.text or "" for t in si.iter(main + "t")) for si in ET.fromstring(z.read("xl/sharedStrings.xml")).findall(main + "si")]
        sheet = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
    table = []
    for row in sheet.iter(main + "row"):
        cells = {}
        for c in row.findall(main + "c"):
            v = c.find(main + "v")
            if v is not None:
                cells[re.match(r"[A-Z]+", c.get("r")).group(0)] = strings[int(v.text)] if c.get("t") == "s" else v.text
        table.append(cells)
    header = {name: col for col, name in table[0].items()}
    return {str(int(float(r[header["id"]]))): r.get(header["genres"]) for r in table[1:] if header["id"] in r}


def split_of(track_id):
    return "dev" if int(hashlib.md5(track_id.encode()).hexdigest(), 16) % 2 == 0 else "test"


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    genre_by_id = xlsx_genres(BEATPORT / "Beatport-EDM-Key-Dataset.xlsx")
    for audio in sorted((BEATPORT / "audio").glob("*.mp3")):
        track_id = audio.stem.split(" ", 1)[0]
        genre = genre_by_id.get(track_id)
        if genre not in GENRE_PRIOR:
            continue
        rows.append({"path": str(audio), "genre": genre, "prior": GENRE_PRIOR[genre], "split": split_of(track_id)})
    (out / "beatport.json").write_text(json.dumps(rows, indent=1))
    (out / "beatport.txt").write_text("\n".join(r["path"] for r in rows) + "\n")

    library = sorted(str(p) for p in LIBRARY.rglob("*") if p.suffix.lower() in {".mp3", ".wav", ".flac"})
    rng = random.Random(100)
    sample = rng.sample(library, 160)
    lossless = [p for p in library if p.lower().endswith((".wav", ".flac"))]
    (out / "user_sample.txt").write_text("\n".join(sample) + "\n")
    (out / "user_lossless.txt").write_text("\n".join(rng.sample(lossless, 30)) + "\n")
    (out / "user_cost.txt").write_text("\n".join(rng.sample(library, 40)) + "\n")
    print(len(rows), "beatport tracks with genre;", sum(r["split"] == "dev" for r in rows), "dev;", len(library), "library tracks")


main()
