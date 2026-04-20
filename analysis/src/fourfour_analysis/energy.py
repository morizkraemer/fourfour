"""Energy level detection via librosa feature fusion."""

import librosa
import numpy as np


def compute_energy(path: str) -> dict | None:
    """Compute energy score (1-10) and label for an audio file.

    Returns dict with keys: score (int 1-10), label ("low"|"medium"|"high").
    Returns None if the track is too short (< 3 seconds) or on failure.
    """
    try:
        y, sr = librosa.load(path, sr=22050)

        if len(y) / sr < 3.0:
            return None

        # Spectral flux (30%)
        stft = np.abs(librosa.stft(y))
        flux = np.mean(np.diff(stft, axis=1) ** 2)

        # Beat strength (25%)
        onset_env = librosa.onset.onset_strength(y=y, sr=sr)
        beat_strength = np.std(onset_env) if len(onset_env) > 0 else 0

        # RMS energy (20%)
        rms = float(np.mean(librosa.feature.rms(y=y)))

        # Spectral centroid (15%)
        centroid = float(np.mean(librosa.feature.spectral_centroid(y=y, sr=sr)))

        # Zero crossing rate (10%)
        zcr = float(np.mean(librosa.feature.zero_crossing_rate(y)))

        # Weighted combination
        raw = (
            0.30 * min(flux / 5.0, 1.0)
            + 0.25 * min(beat_strength / 20.0, 1.0)
            + 0.20 * min(rms / 0.2, 1.0)
            + 0.15 * min(centroid / 5000.0, 1.0)
            + 0.10 * min(zcr / 0.15, 1.0)
        )

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
