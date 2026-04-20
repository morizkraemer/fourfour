"""pytest configuration: mock deeprhythm and reset singletons between tests."""
import sys
from unittest.mock import MagicMock

import pytest

# Create a fake deeprhythm module so bpm.py can be imported without torch
_mock_deeprhythm = MagicMock()
sys.modules.setdefault("deeprhythm", _mock_deeprhythm)


@pytest.fixture(autouse=True)
def reset_bpm_analyzer():
    """Reset the BPM analyzer singleton before each test so mocks take effect cleanly."""
    import fourfour_analysis.bpm as bpm_module
    bpm_module._analyzer = None
    yield
    bpm_module._analyzer = None
