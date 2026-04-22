"""Energy level detection via lightweight waveform statistics."""

import numpy as np
import soundfile as sf


def compute_energy(path: str) -> dict | None:
    """Compute energy score (1-10) and label for an audio file.

    Returns dict with keys: score (int 1-10), label ("low"|"medium"|"high").
    Returns None if the track is too short (< 3 seconds) or on failure.
    """
    try:
        data, sr = sf.read(path, dtype="float32", always_2d=True)
        y = data.mean(axis=1)

        if len(y) / sr < 3.0:
            return None

        rms = float(np.sqrt(np.mean(y**2)))
        peak = float(np.max(np.abs(y)))
        zcr = float(np.mean(np.abs(np.diff(np.signbit(y)))))
        raw = 0.65 * min(rms / 0.25, 1.0) + 0.25 * min(peak / 0.9, 1.0) + 0.10 * min(zcr / 0.2, 1.0)

        score = max(1, min(10, round(raw * 10)))
        if score <= 3:
            label = "low"
        elif score <= 6:
            label = "medium"
        else:
            label = "high"

        return {"score": score, "label": label}
    except Exception:
        return None
