#!/usr/bin/env python3
"""Compare Rekordbox .2EX PWV7 against our generated baseline."""

import json
import struct
from pathlib import Path

REKORDBOX_2EX = Path.home() / "Library/Pioneer/rekordbox/share/PIONEER/USBANLZ/7f8/c4b2a-69bb-41e4-9ad6-898a46744895/ANLZ0000.2EX"
OUR_DATA_JSON = Path("ui/waveform/data.json")

def parse_pwv7(data: bytes):
    """Parse PWV7 section from .2EX bytes. Returns list of [low, mid, high]."""
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
            return entries
        offset += section_len
    return []

def load_our_baseline(path: Path):
    """Load our Lexicon baseline from data.json. Returns list of [low, mid, high]."""
    d = json.loads(path.read_text())
    cols = d.get("waveforms", {}).get("Lexicon (baseline)", [])
    return [[int(c["r"]), int(c["g"]), int(c["b"])] for c in cols]

def stats(name, entries):
    if not entries:
        print(f"{name}: NO DATA")
        return
    lows = [e[0] for e in entries]
    mids = [e[1] for e in entries]
    highs = [e[2] for e in entries]

    def avg(v):
        return sum(v) / len(v)

    def ratio(band, total):
        return avg(band) / avg(total) * 100 if avg(total) > 0 else 0

    total_energy = [e[0] + e[1] + e[2] for e in entries]

    print(f"\n=== {name} ({len(entries)} entries) ===")
    print(f"  Low   — mean: {avg(lows):.1f}  max: {max(lows)}  p90: {sorted(lows)[int(len(lows)*0.9)]:.0f}")
    print(f"  Mid   — mean: {avg(mids):.1f}  max: {max(mids)}  p90: {sorted(mids)[int(len(mids)*0.9)]:.0f}")
    print(f"  High  — mean: {avg(highs):.1f}  max: {max(highs)}  p90: {sorted(highs)[int(len(highs)*0.9)]:.0f}")
    print(f"  Band share — Low: {ratio(lows, total_energy):.1f}%  Mid: {ratio(mids, total_energy):.1f}%  High: {ratio(highs, total_energy):.1f}%")

    # Dominant band per column
    dom_low = sum(1 for e in entries if e[0] >= e[1] and e[0] >= e[2])
    dom_mid = sum(1 for e in entries if e[1] >= e[0] and e[1] >= e[2])
    dom_high = sum(1 for e in entries if e[2] >= e[0] and e[2] >= e[1])
    print(f"  Dominant — Low: {dom_low}  Mid: {dom_mid}  High: {dom_high}")

def compare_sample(rb, ours, n=10):
    """Print n random columns side by side."""
    import random
    print(f"\n=== Sample columns ( Rekordbox | Ours ) ===")
    print(f"{'Idx':>6} | {'RB low':>5} {'mid':>5} {'high':>5} | {'Our low':>5} {'mid':>5} {'high':>5}")
    for idx in random.sample(range(min(len(rb), len(ours))), n):
        r = rb[idx]
        o = ours[idx]
        print(f"{idx:>6} | {r[0]:>5} {r[1]:>5} {r[2]:>5} | {o[0]:>5} {o[1]:>5} {o[2]:>5}")

def main():
    rb = parse_pwv7(REKORDBOX_2EX.read_bytes())
    ours = load_our_baseline(OUR_DATA_JSON)

    stats("Rekordbox", rb)
    stats("Our baseline", ours)
    compare_sample(rb, ours)

    # Hex dump of first 64 Rekordbox columns
    print(f"\n=== Rekordbox PWV7 first 64 bytes (raw hex) ===")
    data = REKORDBOX_2EX.read_bytes()
    offset = 28
    while offset + 24 <= len(data):
        tag = data[offset:offset+4]
        header_len = struct.unpack(">I", data[offset+4:offset+8])[0]
        section_len = struct.unpack(">I", data[offset+8:offset+12])[0]
        if tag == b"PWV7":
            payload = data[offset + header_len : offset + header_len + 64]
            for i in range(0, 64, 16):
                hex_str = ' '.join(f'{b:02x}' for b in payload[i:i+16])
                print(f"  {i:04x}: {hex_str}")
            break
        offset += section_len

if __name__ == "__main__":
    main()
