"""Tests for Lexicon cue point detection."""

import numpy as np
import pytest

from fourfour_analysis.backends.lexicon_cues import detect_sections


def _click_train(bpm: float, duration: float = 30.0, sr: int = 44100) -> tuple[np.ndarray, list[float]]:
    """Generate a click track and its beat positions."""
    total = int(sr * duration)
    audio = np.zeros(total, dtype=np.float32)
    interval = 60.0 / bpm
    click_len = int(sr * 0.01)
    beats = []
    t = 0.0
    i = 0
    while t < duration:
        sample = int(t * sr)
        end = min(sample + click_len, total)
        audio[sample:end] = 0.9
        beats.append(t)
        t += interval
    return audio, beats


def test_returns_start_cue():
    """Any track with enough beats should get a Start cue at time 0."""
    audio, beats = _click_train(128.0, 30.0)
    cues = detect_sections(beats, audio, 44100, 128.0)
    assert len(cues) >= 1
    assert cues[0].label == "Start"
    assert cues[0].time_seconds == 0.0


def test_insufficient_beats_returns_start_only():
    """Too few beats should return only Start."""
    audio = np.zeros(44100 * 5, dtype=np.float32)
    beats = [0.0, 0.5, 1.0, 1.5]  # only 4 beats
    cues = detect_sections(beats, audio, 44100, 128.0)
    assert len(cues) == 1
    assert cues[0].label == "Start"


def test_cue_times_within_track_duration():
    """All cue times should be within track bounds."""
    duration = 30.0
    audio, beats = _click_train(128.0, duration)
    cues = detect_sections(beats, audio, 44100, 128.0)
    for cue in cues:
        assert 0 <= cue.time_seconds <= duration + 1.0, \
            f"Cue '{cue.label}' at {cue.time_seconds}s outside [0, {duration}]"


def test_no_duplicate_start():
    """Start should appear exactly once."""
    audio, beats = _click_train(128.0, 30.0)
    cues = detect_sections(beats, audio, 44100, 128.0)
    starts = [c for c in cues if c.label == "Start"]
    assert len(starts) == 1


def test_zero_bpm_returns_start():
    audio, beats = _click_train(128.0, 10.0)
    cues = detect_sections(beats, audio, 44100, bpm=0)
    assert len(cues) == 1
    assert cues[0].label == "Start"


def test_empty_beats_returns_start():
    audio = np.zeros(44100 * 10, dtype=np.float32)
    cues = detect_sections([], audio, 44100, 128.0)
    assert len(cues) == 1
    assert cues[0].label == "Start"
