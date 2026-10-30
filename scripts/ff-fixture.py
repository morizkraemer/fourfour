#!/usr/bin/env python3
"""Real-size DJ library fixture for the perf suite.

Writes FF_FIXTURE_ROOT/<size>/ (default ~/.cache/ff-fixture):
  pool/      a few hundred distinct synthetic masters (mp3, m4a, flac, wav, aiff), most with cover art
  long/      full-length masters (5-7 min) for the analysis and track-load samples
  music/     <size> files: copies of the masters, each with its own tags and file name
  library/   library.db built from music/ by `bench_startup fixture-lib` (real tag scan, mixed analysed and
             unanalysed, playlists in folders, rating and colour sidecars)
  fixture.json   what was generated; a matching manifest makes a rerun a no-op

Usage: ff-fixture.py <10k|25k|50k|N> --bench <path to bench_startup> [--jobs 12] [--root DIR] [--force]
Needs ffmpeg with libmp3lame, aac, flac and pcm encoders. Standard library only.
"""

import argparse
import concurrent.futures as futures
import json
import os
import random
import shutil
import subprocess
import sys
import time
from pathlib import Path

VERSION = 1
SEED = 4404
SIZES = {"10k": 10_000, "25k": 25_000, "50k": 50_000}
FORMATS = {
    "mp3": {"masters": 150, "weight": 55},
    "m4a": {"masters": 60, "weight": 20},
    "flac": {"masters": 50, "weight": 15},
    "wav": {"masters": 20, "weight": 5},
    "aiff": {"masters": 20, "weight": 5},
}
LONG = [("mp3", 12), ("m4a", 4), ("flac", 4), ("wav", 2), ("aiff", 2)]
ART_SHARE = 0.85
ART_IMAGES = 120
LOOP_SECONDS = 32

GENRES = ["House", "Deep House", "Tech House", "Techno", "Melodic Techno", "Minimal", "Progressive House", "Trance",
          "Drum & Bass", "Jungle", "Breaks", "UK Garage", "Dubstep", "Disco", "Nu Disco", "Funk", "Soul", "Hip-Hop",
          "Afro House", "Amapiano", "Electro", "Ambient", "Downtempo", "Hard Techno", "Italo Disco", ""]
FIRST = ["DJ", "MC", "The", "Lil", "Big", "Dr.", "Kid", "Sister", "Brother", "Mr.", "", "", "", "", "", ""]
NAMES = ["Aurora", "Kölsch", "Mira", "Jürgen", "Sébastien", "Björk", "Nina", "Marco", "Amélie", "Øyvind", "Kenji",
         "Анна", "Дмитрий", "李", "さくら", "Chloé", "Mateo", "Zoë", "Rafael", "Ingrid", "Tobias", "Femi", "Ayo",
         "Laurent", "Dana", "Ibrahim", "Saoirse", "Nikolai", "Priya", "Ólafur", "Ezra", "Kasia", "Mehmet", "Lea"]
LAST = ["Garnier", "Hawtin", "Kraviz", "Müller", "Dubois", "Nakamura", "Petrov", "Okafor", "Silva", "Andersson",
        "O'Brien", "Kowalski", "Rossi", "Haddad", "Schröder", "Lindqvist", "Moreau", "Tanaka", "Novak", "Fischer",
        "Ibáñez", "Larsen", "Costa", "Kaya", "Svensson", "Bauer", "Romero", "Weiß", "Nguyen", "Adeyemi"]
GROUP = ["Collective", "Sound System", "Brothers", "Orchestra", "Project", "Allstars", "Unit", "Experience", "Crew"]
WORDS = ["Midnight", "Velvet", "Echo", "Pulse", "Gravity", "Solar", "Lunar", "Neon", "Paradise", "Shadow", "Fire",
         "Ocean", "Desire", "Signal", "Machine", "Heart", "Dream", "Rhythm", "Motion", "Crystal", "Thunder", "Silk",
         "Horizon", "Fever", "Mirage", "Spiral", "Liquid", "Golden", "Electric", "Hypnotic", "Lost", "Higher", "Deep",
         "Wild", "Sacred", "Broken", "Infinite", "Tokyo", "Berlin", "Lagos", "Detroit", "Ibiza", "Copacabana", "Nacht",
         "Liebe", "Sueño", "Lumière", "Amor", "Город", "夜", "Satellite", "Voodoo", "Jupiter", "Utopia", "Cascade"]
NOUNS = ["Nights", "Groove", "Theory", "Waves", "Dancer", "Station", "Garden", "City", "Ritual", "Code", "Language",
         "Feeling", "Journey", "Tension", "Frequency", "Lights", "Sunrise", "Memories", "Ghosts", "Children", "Forms",
         "Energy", "Strings", "Bells", "Drums", "Steps", "Voices", "Animals", "Promises", "Echoes"]
MIXES = ["Original Mix", "Extended Mix", "Radio Edit", "Dub", "Club Mix", "Instrumental", "VIP", "Rework", "Edit",
         "Remastered 2019", "Live"]
LABELS = [f"{a} {b}" for a in ["Kompakt", "Drumcode", "Defected", "Ninja", "Warp", "Innervisions", "Hessle", "Dirtybird",
                               "Afterlife", "Hospital", "Ostgut", "Running Back", "Toolroom", "Rekids", "Hotflush"]
          for b in ["Records", "Music", "Audio", "Recordings", "", "Black", "White", "Limited"]]
KEYS = ["C", "Cm", "C#", "C#m", "D", "Dm", "Eb", "Ebm", "E", "Em", "F", "Fm", "F#", "F#m", "G", "Gm", "Ab", "Abm",
        "A", "Am", "Bb", "Bbm", "B", "Bm"]


def ffmpeg(args):
    subprocess.run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", *args], check=True)


def synth_expr(bpm, root, minor, seed):
    beat = round(60.0 / bpm, 5)
    third = 1.1892 if minor else 1.2599
    kick = f"sin(2*PI*48*t+7*exp(-mod(t,{beat})*28))*exp(-mod(t,{beat})*7)"
    hat = f"0.12*(random({seed % 7})*2-1)*exp(-mod(t+{beat / 2},{beat})*45)"
    pad = f"0.09*(sin(2*PI*{root}*t)+sin(2*PI*{root * third}*t)+sin(2*PI*{root * 1.4983}*t))*(0.6+0.4*sin(2*PI*t/{beat * 16}))"
    bass = f"0.22*sin(2*PI*{root / 2}*t)*gt(mod(t,{beat}),{beat / 2})"
    left = f"0.55*({kick})+{hat}+{pad}+{bass}"
    right = f"0.55*({kick})+{hat}+{pad.replace('sin(2*PI*t/', 'cos(2*PI*t/')}+{bass}"
    return f"{left}|{right}"


ENCODE = {
    "mp3": lambda r: ["-c:a", "libmp3lame", "-b:a", r.choice(["320k", "320k", "256k", "192k"]), "-id3v2_version", "3"],
    "m4a": lambda r: ["-c:a", "aac", "-b:a", r.choice(["256k", "320k"])],
    "flac": lambda r: ["-c:a", "flac", "-sample_fmt", r.choice(["s16", "s16", "s32"])],
    "wav": lambda r: ["-c:a", "pcm_s16le"],
    "aiff": lambda r: ["-c:a", "pcm_s16be", "-write_id3v2", "1"],
}
ART_FLAGS = {
    "mp3": ["-c:v", "copy", "-disposition:v", "attached_pic"],
    "m4a": ["-c:v", "copy", "-disposition:v", "attached_pic"],
    "flac": ["-c:v", "copy", "-disposition:v", "attached_pic"],
    "aiff": ["-c:v", "copy", "-disposition:v", "attached_pic"],
}


def make_art(path, n):
    size = [500, 600, 600, 800, 1000, 1200][n % 6]
    source = ["testsrc2", "smptehdbars", "rgbtestsrc", "mandelbrot", "cellauto", "life"][n % 6]
    vf = f"scale={size}:{size},hue=h={(n * 37) % 360}:s={1 + (n % 3) * 0.4}"
    if source == "cellauto":
        vf = f"scale={size}:{size}:flags=neighbor,hue=h={(n * 37) % 360}"
    ffmpeg(["-f", "lavfi", "-i", f"{source}=s=320x320:r=1" if source not in ("cellauto", "life") else f"{source}=s=320x320:rate=1",
            "-frames:v", "1", "-vf", vf, "-q:v", str(3 + n % 4), str(path)])


def make_master(path, fmt, seconds, rng, art):
    bpm = rng.choice([rng.uniform(118, 130), rng.uniform(120, 128), rng.uniform(85, 100), rng.uniform(170, 176), rng.uniform(132, 145)])
    root = 55.0 * 2 ** (rng.randrange(12) / 12) * 2
    rate = 48000 if fmt in ("flac", "wav") and rng.random() < 0.2 else 44100
    expr = synth_expr(round(bpm, 2), round(root, 3), rng.random() < 0.6, rng.randrange(1000))
    source = f"aevalsrc=exprs='{expr}':s={rate}:d={min(seconds, LOOP_SECONDS)}"
    if seconds > LOOP_SECONDS:
        loop = path.with_suffix(".loop.wav")
        ffmpeg(["-f", "lavfi", "-i", source, "-c:a", "pcm_f32le", str(loop)])
        args = ["-stream_loop", "-1", "-i", str(loop), "-t", str(seconds)]
    else:
        loop = None
        args = ["-f", "lavfi", "-i", source]
    if art and fmt in ART_FLAGS:
        args += ["-i", str(art), "-map", "0:a", "-map", "1:v", *ART_FLAGS[fmt]]
    args += ["-af", "volume=0.8", *ENCODE[fmt](rng), "-ar", str(rate), str(path)]
    ffmpeg(args)
    if loop:
        loop.unlink()
    return {"path": str(path), "format": fmt, "seconds": seconds, "bpm": round(bpm, 2), "art": bool(art and fmt in ART_FLAGS)}


class Names:
    def __init__(self, rng, count):
        self.rng = rng
        self.artists = []
        seen = set()
        while len(self.artists) < max(40, count // 6):
            shape = rng.random()
            if shape < 0.45:
                name = f"{rng.choice(NAMES)} {rng.choice(LAST)}"
            elif shape < 0.65:
                name = f"{rng.choice(FIRST)} {rng.choice(WORDS)}".strip()
            elif shape < 0.8:
                name = f"{rng.choice(WORDS)} {rng.choice(GROUP)}"
            elif shape < 0.9:
                name = f"{rng.choice(WORDS).upper()}{rng.choice(['', ' ', '-'])}{rng.choice(NOUNS).upper()}"
            else:
                name = f"{rng.choice(NAMES)} & {rng.choice(NAMES)}"
            if name not in seen:
                seen.add(name)
                self.artists.append(name)
        self.titles = set()

    def track(self, i):
        rng = self.rng
        artist = self.artists[int(len(self.artists) * rng.random() ** 1.6)]
        if rng.random() < 0.15:
            artist = f"{artist} feat. {rng.choice(self.artists)}"
        while True:
            base = rng.choice([f"{rng.choice(WORDS)} {rng.choice(NOUNS)}", rng.choice(WORDS), f"{rng.choice(NOUNS)} of {rng.choice(WORDS)}",
                               f"{rng.choice(WORDS)} {rng.choice(WORDS)} {rng.choice(NOUNS)}"])
            if rng.random() < 0.05:
                base += " " + " ".join(rng.choice(WORDS) for _ in range(6))
            mix = rng.random()
            if mix < 0.35:
                base += f" ({rng.choice(MIXES)})"
            elif mix < 0.55:
                base += f" ({rng.choice(self.artists)} Remix)"
            key = (artist.lower(), base.lower())
            if key not in self.titles:
                self.titles.add(key)
                return artist, base


def pick_format(rng):
    total = sum(f["weight"] for f in FORMATS.values())
    roll = rng.uniform(0, total)
    for fmt, spec in FORMATS.items():
        roll -= spec["weight"]
        if roll <= 0:
            return fmt
    return "mp3"


def safe(text):
    out = "".join(c if c not in '/\\:*?"<>|' else "_" for c in text).strip(". ")
    return out[:80] or "_"


def tag_copy(job):
    src, dst, fmt, tags = job
    args = ["-i", src, "-map", "0", "-c", "copy", "-map_metadata", "-1"]
    for k, v in tags.items():
        if v != "":
            args += ["-metadata", f"{k}={v}"]
    if fmt == "mp3":
        args += ["-id3v2_version", "3"]
    if fmt == "aiff":
        args += ["-write_id3v2", "1"]
    args.append(dst)
    ffmpeg(args)


def run_pool(fn, jobs, workers, label):
    done = 0
    started = time.time()
    with futures.ThreadPoolExecutor(workers) as pool:
        for _ in pool.map(fn, jobs):
            done += 1
            if done % 1000 == 0 or done == len(jobs):
                print(f"[fixture] {label} {done}/{len(jobs)} ({time.time() - started:.0f} s)", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("size")
    ap.add_argument("--bench", required=True)
    ap.add_argument("--root", default=os.environ.get("FF_FIXTURE_ROOT", str(Path.home() / ".cache/ff-fixture")))
    ap.add_argument("--jobs", type=int, default=min(12, os.cpu_count() or 4))
    ap.add_argument("--analysed", type=float, default=0.7)
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()

    count = SIZES.get(a.size) or int(a.size)
    root = Path(a.root).expanduser()
    out = root / (a.size if a.size in SIZES else str(count))
    manifest_path = out / "fixture.json"
    wanted = {"version": VERSION, "seed": SEED, "tracks": count, "analysed": a.analysed}
    if manifest_path.exists() and not a.force:
        have = json.loads(manifest_path.read_text())
        if all(have.get(k) == v for k, v in wanted.items()) and (out / "library/library.db").exists():
            print(f"[fixture] {out} is up to date", file=sys.stderr)
            print(out)
            return
    started = time.time()
    rng = random.Random(SEED)
    pool_dir = root / f"pool-v{VERSION}"
    art_dir, master_dir, long_dir = pool_dir / "art", pool_dir / "masters", pool_dir / "long"
    for d in (art_dir, master_dir, long_dir):
        d.mkdir(parents=True, exist_ok=True)

    pool_manifest = pool_dir / "pool.json"
    if pool_manifest.exists():
        pool = json.loads(pool_manifest.read_text())
    else:
        arts = [art_dir / f"cover{n:03d}.jpg" for n in range(ART_IMAGES)]
        run_pool(lambda n: make_art(arts[n], n), list(range(ART_IMAGES)), a.jobs, "cover art")
        jobs = []
        for fmt, spec in FORMATS.items():
            for n in range(spec["masters"]):
                art = arts[rng.randrange(ART_IMAGES)] if rng.random() < ART_SHARE else None
                jobs.append((master_dir / f"{fmt}{n:03d}.{fmt}", fmt, rng.choice([7, 8, 9, 10, 11, 12, 13]), random.Random(rng.random()), art))
        for fmt, n in LONG:
            for k in range(n):
                art = arts[rng.randrange(ART_IMAGES)]
                jobs.append((long_dir / f"long-{fmt}{k:02d}.{fmt}", fmt, rng.randrange(300, 421), random.Random(rng.random()), art))
        masters = []
        run_pool(lambda j: masters.append(make_master(*j)), jobs, a.jobs, "masters")
        pool = {"masters": sorted([m for m in masters if "/long/" not in m["path"]], key=lambda m: m["path"]),
                "long": sorted([m for m in masters if "/long/" in m["path"]], key=lambda m: m["path"])}
        pool_manifest.write_text(json.dumps(pool, indent=1))

    by_format = {}
    for m in pool["masters"]:
        by_format.setdefault(m["format"], []).append(m)

    music = out / "music"
    if music.exists():
        shutil.rmtree(music)
    music.mkdir(parents=True)
    names = Names(rng, count)
    albums = {}
    jobs = []
    copies = count - len(pool["long"])
    for i in range(copies):
        fmt = pick_format(rng)
        master = by_format[fmt][rng.randrange(len(by_format[fmt]))]
        artist, title = names.track(i)
        genre = rng.choice(GENRES)
        album_artist = artist.split(" feat. ")[0]
        compilation = rng.random() < 0.12
        album_key = ("VA" if compilation else album_artist, rng.randrange(4))
        album = albums.setdefault(album_key, f"{rng.choice(WORDS)} {rng.choice(NOUNS)}" + (" Vol. %d" % rng.randrange(1, 12) if compilation else ""))
        tags = {
            "title": title,
            "artist": artist,
            "album": album if rng.random() < 0.9 else "",
            "album_artist": "Various Artists" if compilation else "",
            "genre": genre,
            "date": str(rng.randrange(1975, 2027)) if rng.random() < 0.85 else "",
            "track": str(rng.randrange(1, 15)) if rng.random() < 0.8 else "",
            "publisher": rng.choice(LABELS).strip() if rng.random() < 0.6 else "",
            "comment": rng.choice(["", "", "Purchased at Beatport", "Bandcamp", "promo - do not share", f"{rng.choice(KEYS)} - {rng.randrange(90, 175)} BPM", "Energy 7"]),
            "TBPM" if fmt == "mp3" else "bpm": str(round(master["bpm"])) if rng.random() < 0.3 else "",
            "TKEY" if fmt == "mp3" else "initialkey": rng.choice(KEYS) if rng.random() < 0.25 else "",
        }
        folder = music / safe(genre or "Unsorted") / safe(album_artist[:1].upper() or "_") / safe(album_artist)
        name = f"{int(tags['track']):02d} " if tags["track"] else ""
        dst = folder / f"{name}{safe(artist)} - {safe(title)}.{fmt}"
        jobs.append((master["path"], str(dst), fmt, tags))
    for d in {Path(j[1]).parent for j in jobs}:
        d.mkdir(parents=True, exist_ok=True)
    run_pool(tag_copy, jobs, a.jobs, "tagged copies")
    long_dst = music / "_Long Mixes"
    long_dst.mkdir()
    long_jobs = []
    for n, m in enumerate(pool["long"]):
        artist, title = names.track(copies + n)
        long_jobs.append((m["path"], str(long_dst / f"{safe(artist)} - {safe(title)}.{m['format']}"), m["format"],
                          {"title": title, "artist": artist, "genre": rng.choice(GENRES), "date": "2024"}))
    run_pool(tag_copy, long_jobs, a.jobs, "long copies")
    (out / "long.txt").write_text("".join(j[1] + "\n" for j in long_jobs))

    lib = out / "library"
    if lib.exists():
        shutil.rmtree(lib)
    print(f"[fixture] building the library from {music}", file=sys.stderr)
    subprocess.run([a.bench, "fixture-lib", str(music), str(lib), str(a.analysed), str(SEED)], check=True)

    size_bytes = sum(f.stat().st_size for f in music.rglob("*") if f.is_file())
    manifest = {**wanted, "files": len(jobs) + len(long_jobs), "music_bytes": size_bytes, "seconds": round(time.time() - started),
                "pool": str(pool_dir), "formats": {f: sum(1 for j in jobs if j[2] == f) for f in FORMATS}}
    manifest_path.write_text(json.dumps(manifest, indent=1))
    print(f"[fixture] {out}: {manifest['files']} files, {size_bytes / 1e9:.1f} GB, {manifest['seconds']} s", file=sys.stderr)
    print(out)


if __name__ == "__main__":
    main()
