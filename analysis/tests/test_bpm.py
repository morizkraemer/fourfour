from unittest.mock import patch


def test_detect_bpm_returns_float():
    """BPM detection should return a float tempo value."""
    with patch("fourfour_analysis.bpm.FinalStackBackend") as MockBackend:
        instance = MockBackend.return_value
        instance.analyze_track.return_value.bpm = 128.0

        from fourfour_analysis.bpm import detect_bpm
        result = detect_bpm("/fake/track.mp3")

        assert result == 128.0
        instance.analyze_track.assert_called_once_with("/fake/track.mp3")


def test_detect_bpm_octave_correction_low():
    """BPM below 70 should be doubled."""
    with patch("fourfour_analysis.bpm.FinalStackBackend") as MockBackend:
        instance = MockBackend.return_value
        instance.analyze_track.return_value.bpm = 64.0

        from fourfour_analysis.bpm import detect_bpm
        result = detect_bpm("/fake/track.mp3")

        assert result == 128.0


def test_detect_bpm_octave_correction_high():
    """BPM above 200 should be halved."""
    with patch("fourfour_analysis.bpm.FinalStackBackend") as MockBackend:
        instance = MockBackend.return_value
        instance.analyze_track.return_value.bpm = 256.0

        from fourfour_analysis.bpm import detect_bpm
        result = detect_bpm("/fake/track.mp3")

        assert result == 128.0


def test_detect_bpm_returns_none_on_failure():
    """Should return None if analysis fails."""
    with patch("fourfour_analysis.bpm.FinalStackBackend") as MockBackend:
        instance = MockBackend.return_value
        instance.analyze_track.side_effect = RuntimeError("decode failed")

        from fourfour_analysis.bpm import detect_bpm
        result = detect_bpm("/fake/track.mp3")

        assert result is None
