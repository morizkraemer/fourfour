#!/usr/bin/env python3
"""Re-encode a list of lossless tracks into variants for the energy stability test (r100).

usage: energy_reencode.py <list.txt> <out_dir>

Writes <out_dir>/<variant>/<stem>.<ext> and <out_dir>/<variant>.txt (one path per line) for
mp3_128 (libmp3lame 128 kbps), aac_128 (aac 128 kbps) and gain_m6 (flac, -6 dB). Single-threaded ffmpeg.
"""
import subprocess
import sys
from pathlib import Path

VARIANTS = {
    "mp3_128": ("mp3", ["-c:a", "libmp3lame", "-b:a", "128k"]),
    "aac_128": ("m4a", ["-c:a", "aac", "-b:a", "128k"]),
    "gain_m6": ("flac", ["-af", "volume=-6dB", "-c:a", "flac"]),
}


def main():
    sources = [Path(p) for p in Path(sys.argv[1]).read_text().split("\n") if p.strip()]
    out = Path(sys.argv[2])
    for name, (ext, args) in VARIANTS.items():
        folder = out / name
        folder.mkdir(parents=True, exist_ok=True)
        done = []
        for src in sources:
            dst = folder / f"{src.stem}.{ext}"
            if not dst.exists():
                subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-threads", "1", "-i", str(src), "-vn", *args, str(dst)], check=True)
            done.append(str(dst))
        (out / f"{name}.txt").write_text("\n".join(done) + "\n")
        print(name, len(done))


main()
