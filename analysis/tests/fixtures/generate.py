"""Test fixtures — generated audio files for unit tests."""

import numpy as np
import soundfile as sf
from pathlib import Path


def generate_fixtures(output_dir: Path) -> dict[str, Path]:
    """Generate standard test audio fixtures.

    Returns dict mapping fixture name to file path.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    fixtures = {}

    # 10-second 440Hz sine at 44100Hz
    sr = 44100
    t = np.arange(int(sr * 10), dtype=np.float32) / sr
    sine = (np.sin(2 * np.pi * 440 * t) * 0.8).astype(np.float32)
    path = output_dir / "sine_440_10s.wav"
    sf.write(str(path), sine, sr)
    fixtures["sine_440_10s"] = path

    # Click track at 128 BPM, 10 seconds
    duration = 10.0
    total_samples = int(sr * duration)
    audio = np.zeros(total_samples, dtype=np.float32)
    beat_interval = int(sr * 60.0 / 128.0)
    click_len = int(sr * 0.01)  # 10ms impulse
    for i in range(0, total_samples, beat_interval):
        end = min(i + click_len, total_samples)
        audio[i:end] = 0.9
    path = output_dir / "click_track_128bpm.wav"
    sf.write(str(path), audio, sr)
    fixtures["click_track_128bpm"] = path

    return fixtures


if __name__ == "__main__":
    import sys
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("tests/fixtures")
    fixtures = generate_fixtures(out)
    for name, path in fixtures.items():
        print(f"  {name}: {path}")
