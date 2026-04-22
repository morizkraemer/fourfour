from fourfour_analysis.key import detect_key, to_camelot


def test_to_camelot_a_minor():
    assert to_camelot("A minor") == "8A"


def test_to_camelot_c_major():
    assert to_camelot("C major") == "12B"


def test_to_camelot_unknown_returns_none():
    assert to_camelot("X augmented") is None


def test_detect_key_returns_camelot_string():
    """detect_key should return a Camelot string like '8A'."""
    from unittest.mock import patch

    with patch("fourfour_analysis.key.EssentiaKeyBackend") as MockBackend:
        MockBackend.return_value.analyze_track.return_value.key = "8A"
        result = detect_key("/fake/track.mp3")

    assert result == "8A"
