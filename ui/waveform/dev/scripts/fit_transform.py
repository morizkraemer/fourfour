#!/usr/bin/env python3
"""Learn the optimal per-band transform from our filter bank to Rekordbox PWV7."""

import json
import struct
from pathlib import Path
import numpy as np
from numpy.polynomial import polynomial as P

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

    import sys
    sys.path.insert(0, str(Path(__file__).parent / "analysis/src"))
    from fourfour_analysis.audio_io import load_audio, preprocess_waveform
    from fourfour_analysis.backends.lexicon_waveform import generate_waveform_filterbank, FilterbankParams

    audio, sr = load_audio("Hoodlum/Hoodlum - Traumer.flac")
    audio_12k, sr_12k = preprocess_waveform(audio, sr)

    p = FilterbankParams(low_cutoff=70.0, mid_cutoff=1500.0, measure="rms", filter_order=8, scale_mode="per_track_95")
    cols = generate_waveform_filterbank(audio_12k, sr_12k, p)
    ours = np.array([[c.r, c.g, c.b] for c in cols], dtype=np.float64)
    rb_arr = np.array(rb[:len(ours)], dtype=np.float64)

    print(f"Fitting transforms on {len(ours)} samples...\n")

    for band_idx, band_name in enumerate(["Low", "Mid", "High"]):
        x = ours[:, band_idx]
        y = rb_arr[:, band_idx]

        # Only fit where both have signal
        mask = (x > 0) & (y > 0)
        x_fit = x[mask]
        y_fit = y[mask]

        # 1. Linear: y = a*x + b
        A = np.vstack([x_fit, np.ones(len(x_fit))]).T
        a_lin, b_lin = np.linalg.lstsq(A, y_fit, rcond=None)[0]
        pred_lin = a_lin * x + b_lin
        mse_lin = np.mean((pred_lin - y)**2)

        # 2. Power law: y = a * x^p  (fit in log space)
        logx = np.log(x_fit + 1)
        logy = np.log(y_fit + 1)
        A_log = np.vstack([logx, np.ones(len(logx))]).T
        p_pow, loga_pow = np.linalg.lstsq(A_log, logy, rcond=None)[0]
        a_pow = np.exp(loga_pow)
        pred_pow = a_pow * (x + 1) ** p_pow - 1
        pred_pow = np.clip(pred_pow, 0, 255)
        mse_pow = np.mean((pred_pow - y)**2)

        # 3. Quadratic: y = a*x^2 + b*x + c
        coeffs_quad, _ = P.polyfit(x_fit, y_fit, 2, full=True)
        pred_quad = P.polyval(x, coeffs_quad)
        pred_quad = np.clip(pred_quad, 0, 255)
        mse_quad = np.mean((pred_quad - y)**2)

        # 4. Cubic: y = a*x^3 + b*x^2 + c*x + d
        coeffs_cubic, _ = P.polyfit(x_fit, y_fit, 3, full=True)
        pred_cubic = P.polyval(x, coeffs_cubic)
        pred_cubic = np.clip(pred_cubic, 0, 255)
        mse_cubic = np.mean((pred_cubic - y)**2)

        # 5. Percentile mapping (non-parametric)
        percentiles = np.linspace(0, 100, 101)
        our_pcts = np.percentile(x, percentiles)
        rb_pcts = np.percentile(y, percentiles)
        pred_pct = np.interp(x, our_pcts, rb_pcts)
        mse_pct = np.mean((pred_pct - y)**2)

        print(f"=== {band_name} band ===")
        print(f"  Linear:     y = {a_lin:.3f}*x + {b_lin:.2f}     MSE={mse_lin:.1f}")
        print(f"  Power:      y = {a_pow:.3f} * (x+1)^{p_pow:.3f} - 1   MSE={mse_pow:.1f}")
        print(f"  Quadratic:  MSE={mse_quad:.1f}")
        print(f"  Cubic:      MSE={mse_cubic:.1f}")
        print(f"  Percentile: MSE={mse_pct:.1f}")

        # Show sample transforms
        print(f"  Sample: our=10→{np.interp(10, our_pcts, rb_pcts):.0f}, our=50→{np.interp(50, our_pcts, rb_pcts):.0f}, our=100→{np.interp(100, our_pcts, rb_pcts):.0f}")
        print()

    # Save percentile mapping tables for use in code
    mapping = {}
    for band_idx, band_name in enumerate(["low", "mid", "high"]):
        x = ours[:, band_idx]
        y = rb_arr[:, band_idx]
        percentiles = np.linspace(0, 100, 101)
        our_pcts = np.percentile(x, percentiles)
        rb_pcts = np.percentile(y, percentiles)
        mapping[band_name] = {
            "our_pcts": our_pcts.tolist(),
            "rb_pcts": rb_pcts.tolist(),
        }

    out = Path("waveform_mapping.json")
    out.write_text(json.dumps(mapping, indent=2))
    print(f"Saved percentile mapping tables to {out}")

if __name__ == "__main__":
    main()
