"""Waveform extraction: peaks, color bands, and Pioneer PWAV format."""

import numpy as np
import soundfile as sf


def extract_peaks(path: str, target_points: int = 2000) -> list[tuple[float, float]]:
    """Extract (min, max) amplitude pairs by chunking the audio.

    Returns exactly `target_points` pairs. Each pair represents
    the min and max sample amplitude in that time window.
    """
    with sf.SoundFile(path) as f:
        total_frames = len(f)
        chunk_size = max(1, total_frames // target_points)
        peaks = []
        while len(peaks) < target_points:
            block = f.read(chunk_size, dtype="float32", always_2d=True)
            if not len(block):
                break
            mono = block.mean(axis=1)
            peaks.append((float(mono.min()), float(mono.max())))

    # Pad or trim to exact target
    while len(peaks) < target_points:
        peaks.append((0.0, 0.0))
    return peaks[:target_points]


def extract_color_bands(path: str, points: int = 2000) -> list[dict]:
    """Extract 3-band FFT color data (bass=R, mids=G, highs=B).

    Returns `points` dicts with keys: amp, r, g, b (all 0.0-1.0 normalized).
    """
    data, sr = sf.read(path, dtype="float32", always_2d=True)
    mono = data.mean(axis=1)
    n_fft = 2048
    hop = max(1, len(mono) // points)

    results = []
    for i in range(min(points, len(mono) // hop)):
        chunk = mono[i * hop : (i + 1) * hop]
        if len(chunk) < n_fft:
            chunk = np.pad(chunk, (0, n_fft - len(chunk)))
        spec = np.abs(np.fft.rfft(chunk, n=n_fft))
        freqs = np.fft.rfftfreq(n_fft, 1 / sr)

        bass = float(spec[(freqs >= 20) & (freqs < 250)].mean())
        mids = float(spec[(freqs >= 250) & (freqs < 4000)].mean())
        highs = float(spec[(freqs >= 4000)].mean())
        amp = float(np.abs(chunk).max())

        results.append({"amp": amp, "r": bass, "g": mids, "b": highs})

    # Pad to exact count
    while len(results) < points:
        results.append({"amp": 0.0, "r": 0.0, "g": 0.0, "b": 0.0})

    # Normalize each channel to 0.0-1.0
    max_amp = max(r["amp"] for r in results) or 1.0
    max_r = max(r["r"] for r in results) or 1.0
    max_g = max(r["g"] for r in results) or 1.0
    max_b = max(r["b"] for r in results) or 1.0

    for r in results:
        r["amp"] = r["amp"] / max_amp
        r["r"] = r["r"] / max_r
        r["g"] = r["g"] / max_g
        r["b"] = r["b"] / max_b

    return results[:points]


def generate_pioneer_3band(path: str) -> dict:
    """Generate Pioneer-native 3-band waveform data.

    Returns dict with:
        detail: list of [low, mid, high] (0-255 each), entry count = duration * 150
        overview: list of [low, mid, high] (0-255 each), always 1200 entries
    """
    info = sf.info(path)
    duration = info.duration
    sr = info.samplerate

    data, _ = sf.read(path, dtype="float32", always_2d=True)
    mono = data.mean(axis=1)

    n_fft = 2048
    detail_count = int(duration * 150)
    overview_count = 1200

    def compute_bands(audio_data, num_entries):
        hop = max(1, len(audio_data) // num_entries)
        freqs = np.fft.rfftfreq(n_fft, 1.0 / sr)

        low_mask = (freqs >= 20) & (freqs < 250)
        mid_mask = (freqs >= 250) & (freqs < 4000)
        high_mask = freqs >= 4000

        raw = []
        max_low = max_mid = max_high = 0.0

        for i in range(min(num_entries, len(audio_data) // hop)):
            chunk = audio_data[i * hop : (i + 1) * hop]
            if len(chunk) < n_fft:
                chunk = np.pad(chunk, (0, n_fft - len(chunk)))
            spec = np.abs(np.fft.rfft(chunk, n=n_fft))

            low = float(spec[low_mask].mean()) if low_mask.any() else 0.0
            mid = float(spec[mid_mask].mean()) if mid_mask.any() else 0.0
            high = float(spec[high_mask].mean()) if high_mask.any() else 0.0

            raw.append((low, mid, high))
            max_low = max(max_low, low)
            max_mid = max(max_mid, mid)
            max_high = max(max_high, high)

        # Normalize to Pioneer's typical ranges
        entries = []
        for low, mid, high in raw:
            l = int((low / max_low * 0x60) if max_low > 0 else 0)
            m = int((mid / max_mid * 0x40) if max_mid > 0 else 0)
            h = int((high / max_high * 0x30) if max_high > 0 else 0)
            entries.append([min(255, l), min(255, m), min(255, h)])

        # Pad to exact count
        while len(entries) < num_entries:
            entries.append([0, 0, 0])
        return entries[:num_entries]

    detail = compute_bands(mono, detail_count)
    overview = compute_bands(mono, overview_count)

    return {"detail": detail, "overview": overview}


def generate_pwav_preview(path: str) -> bytes:
    """Generate a 400-byte Pioneer PWAV preview.

    Each byte encodes:
    - bits 0-4: height (0-31)
    - bits 5-7: whiteness/intensity (0-7)

    This matches the format in pioneer-usb-writer WaveformPreview.
    """
    data, sr = sf.read(path, dtype="float32", always_2d=True)
    mono = data.mean(axis=1)

    chunk_size = max(1, len(mono) // 400)
    rms_values = np.zeros(400, dtype=np.float32)

    for i in range(400):
        start = i * chunk_size
        end = min(start + chunk_size, len(mono))
        if start >= len(mono):
            break
        chunk = mono[start:end]
        rms_values[i] = np.sqrt(np.mean(chunk**2))

    max_rms = rms_values.max()
    preview = bytearray(400)

    if max_rms > 0:
        for i in range(400):
            normalized = rms_values[i] / max_rms
            height = int(normalized * 31)  # 5 bits: 0-31
            whiteness = int(normalized * 7)  # 3 bits: 0-7
            preview[i] = (whiteness << 5) | (height & 0x1F)

    return bytes(preview)
