#!/usr/bin/env python3
"""Direct binary comparison: Rekordbox .2EX vs our generated .2EX."""

import json
import struct
from pathlib import Path
import numpy as np

REKORDBOX_2EX = Path.home() / "Library/Pioneer/rekordbox/share/PIONEER/USBANLZ/7f8/c4b2a-69bb-41e4-9ad6-898a46744895/ANLZ0000.2EX"

def parse_pwv7(data: bytes):
    """Parse PWV7 section from .2EX. Returns (entries, header_offset, section_len)."""
    offset = 28  # skip PMAI header
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
            return entries, offset, section_len
        offset += section_len
    return [], 0, 0

def main():
    rb_data = REKORDBOX_2EX.read_bytes()
    rb_entries, rb_off, rb_len = parse_pwv7(rb_data)
    print(f"Rekordbox PWV7: {len(rb_entries)} entries at offset {rb_off}, section len {rb_len}")

    # Load our best filter bank
    import sys
    sys.path.insert(0, str(Path(__file__).parent / "analysis/src"))
    from fourfour_analysis.audio_io import load_audio, preprocess_waveform
    from fourfour_analysis.backends.lexicon_waveform import generate_waveform_filterbank, FilterbankParams

    audio, sr = load_audio("Hoodlum/Hoodlum - Traumer.flac")
    audio_12k, sr_12k = preprocess_waveform(audio, sr)

    # Use default params (calibrated against 5 Rekordbox-analyzed tracks)
    params = FilterbankParams()
    cols = generate_waveform_filterbank(audio_12k, sr_12k, params)
    our_entries = [[c.r, c.g, c.b] for c in cols]

    # Match lengths
    n = min(len(rb_entries), len(our_entries))
    rb_entries = rb_entries[:n]
    our_entries = our_entries[:n]

    print(f"\nComparing {n} entries")

    # Per-band differences
    rb_low = np.array([e[0] for e in rb_entries], dtype=np.float64)
    rb_mid = np.array([e[1] for e in rb_entries], dtype=np.float64)
    rb_high = np.array([e[2] for e in rb_entries], dtype=np.float64)

    our_low = np.array([e[0] for e in our_entries], dtype=np.float64)
    our_mid = np.array([e[1] for e in our_entries], dtype=np.float64)
    our_high = np.array([e[2] for e in our_entries], dtype=np.float64)

    print(f"\n=== Mean absolute error ===")
    print(f"  Low:  {np.mean(np.abs(rb_low - our_low)):.1f}")
    print(f"  Mid:  {np.mean(np.abs(rb_mid - our_mid)):.1f}")
    print(f"  High: {np.mean(np.abs(rb_high - our_high)):.1f}")

    print(f"\n=== Mean squared error ===")
    print(f"  Low:  {np.mean((rb_low - our_low)**2):.1f}")
    print(f"  Mid:  {np.mean((rb_mid - our_mid)**2):.1f}")
    print(f"  High: {np.mean((rb_high - our_high)**2):.1f}")

    print(f"\n=== Correlation ===")
    print(f"  Low:  {np.corrcoef(rb_low, our_low)[0,1]:.3f}")
    print(f"  Mid:  {np.corrcoef(rb_mid, our_mid)[0,1]:.3f}")
    print(f"  High: {np.corrcoef(rb_high, our_high)[0,1]:.3f}")

    print(f"\n=== Ratio analysis (Low/Mid/High means) ===")
    print(f"  Rekordbox: {np.mean(rb_low):.1f} / {np.mean(rb_mid):.1f} / {np.mean(rb_high):.1f}")
    print(f"  Ours:      {np.mean(our_low):.1f} / {np.mean(our_mid):.1f} / {np.mean(our_high):.1f}")

    # Find columns with biggest divergence
    diffs = np.abs(rb_low - our_low) + np.abs(rb_mid - our_mid) + np.abs(rb_high - our_high)
    worst_idx = np.argsort(diffs)[-20:][::-1]

    print(f"\n=== 20 worst-matching columns ===")
    print(f"{'Idx':>6} | {'RB low':>4} {'mid':>4} {'high':>4} | {'Our low':>4} {'mid':>4} {'high':>4} | {'Diff':>4}")
    for idx in worst_idx:
        rb = rb_entries[idx]
        ours = our_entries[idx]
        d = int(diffs[idx])
        print(f"{idx:>6} | {rb[0]:>4} {rb[1]:>4} {rb[2]:>4} | {ours[0]:>4} {ours[1]:>4} {ours[2]:>4} | {d:>4}")

    # Distribution histograms
    print(f"\n=== Low band value distribution ===")
    print("Bin    | Rekordbox | Ours")
    for b in range(0, 256, 16):
        rb_c = sum(1 for v in rb_low if b <= v < b+16)
        our_c = sum(1 for v in our_low if b <= v < b+16)
        print(f"{b:>3}-{b+15:>3} | {rb_c:>9} | {our_c:>4}")

    print(f"\n=== Mid band value distribution ===")
    print("Bin    | Rekordbox | Ours")
    for b in range(0, 256, 16):
        rb_c = sum(1 for v in rb_mid if b <= v < b+16)
        our_c = sum(1 for v in our_mid if b <= v < b+16)
        print(f"{b:>3}-{b+15:>3} | {rb_c:>9} | {our_c:>4}")

if __name__ == "__main__":
    main()
