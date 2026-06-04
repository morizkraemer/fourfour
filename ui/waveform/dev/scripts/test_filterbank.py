#!/usr/bin/env python3
"""Quick test: one filter bank variant vs Rekordbox PWV7."""

import json
import struct
from pathlib import Path
import numpy as np

REKORDBOX_2EX = Path.home() / "Library/Pioneer/rekordbox/share/PIONEER/USBANLZ/7f8/c4b2a-69bb-41e4-9ad6-898a46744895/ANLZ0000.2EX"

def parse_pwv7(data: bytes):
    offset = 28
    while offset + 24 <= len(data):
        tag = data[offset:offset+4]
        header_len = struct.unpack(">I", data[offset+4:offset+8])[0]
        section_len = struct.unpack(">I", data[offset+8:offset+12])[0]
        if tag == b"PWV7":
            entry_count = struct.unpack(">I", data[offset+16:offset+20])[0]
            payload = data[offset + header_len : offset + section_len]
            entries = []
            for i in range(entry_count):
                base = i * 3
                if base + 3 > len(payload):
                    break
                entries.append([payload[base], payload[base+1], payload[base+2]])
            return entries
        offset += section_len
    return []

def main():
    rb = parse_pwv7(REKORDBOX_2EX.read_bytes())

    # Import and run filter bank
    import sys
    sys.path.insert(0, str(Path(__file__).parent / "analysis/src"))
    from fourfour_analysis.audio_io import load_audio, preprocess_waveform
    from fourfour_analysis.backends.lexicon_waveform import generate_waveform_filterbank, FilterbankParams

    audio, sr = load_audio("Hoodlum/Hoodlum - Traumer.flac")
    audio_12k, sr_12k = preprocess_waveform(audio, sr)

    params = FilterbankParams(low_cutoff=120.0, mid_cutoff=1500.0, measure="max", scale_mode="global")
    cols = generate_waveform_filterbank(audio_12k, sr_12k, params)

    ours = [[c.r, c.g, c.b] for c in cols]

    def stats(name, entries):
        lows = [e[0] for e in entries]
        mids = [e[1] for e in entries]
        highs = [e[2] for e in entries]
        total = [sum(e) for e in entries]
        dom_low = sum(1 for e in entries if e[0] >= e[1] and e[0] >= e[2])
        print(f"\n{name} ({len(entries)} cols)")
        print(f"  Low  mean={np.mean(lows):.1f} max={max(lows)} p90={np.percentile(lows,90):.0f}")
        print(f"  Mid  mean={np.mean(mids):.1f} max={max(mids)} p90={np.percentile(mids,90):.0f}")
        print(f"  High mean={np.mean(highs):.1f} max={max(highs)} p90={np.percentile(highs,90):.0f}")
        print(f"  Share L/M/H: {np.mean(lows)/np.mean(total)*100:.1f}% / {np.mean(mids)/np.mean(total)*100:.1f}% / {np.mean(highs)/np.mean(total)*100:.1f}%")
        print(f"  Dominant low: {dom_low}/{len(entries)} ({dom_low/len(entries)*100:.0f}%)")

    stats("Rekordbox", rb)
    stats("Filterbank 120/1.5k", ours)

    # Random samples
    import random
    print("\nSample columns:")
    for idx in random.sample(range(min(len(rb), len(ours))), 10):
        print(f"  {idx:5d}: RB={rb[idx]}  FB={ours[idx]}")

if __name__ == "__main__":
    main()
