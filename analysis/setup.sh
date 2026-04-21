#!/usr/bin/env bash
# Setup script for fourfour-analysis Python environment
# Run once: ./analysis/setup.sh
# Then use: ./analysis/run.sh analyze track.mp3 --json

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

echo "Setting up fourfour-analysis..."

# Create venv if it doesn't exist
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment..."
    python3.11 -m venv "$VENV_DIR" 2>/dev/null || python3 -m venv "$VENV_DIR"
fi

echo "Installing dependencies (this may take a while — torch is ~400MB)..."
"$VENV_DIR/bin/pip" install --upgrade pip -q
"$VENV_DIR/bin/pip" install -e "$SCRIPT_DIR" -q

# Optional: pysqlcipher3 for benchmark mode
if command -v brew &>/dev/null && brew list sqlcipher &>/dev/null; then
    echo "Installing pysqlcipher3 for benchmark mode..."
    "$VENV_DIR/bin/pip" install pysqlcipher3 -q 2>/dev/null || echo "  (pysqlcipher3 install failed — benchmark mode won't work, but analysis is fine)"
else
    echo "Note: sqlcipher not found. For benchmark mode: brew install sqlcipher && ./setup.sh"
fi

echo ""
echo "Done! Use ./analysis/run.sh to run the analyzer:"
echo "  ./analysis/run.sh analyze track.mp3 --json"
echo "  ./analysis/run.sh analyze --dir ~/Music/djset/ --workers 8"
echo "  ./analysis/run.sh benchmark --playlist 'Test Set'"
