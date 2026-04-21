#!/usr/bin/env bash
# Run the fourfour-analyze CLI using the local venv.
# Usage:
#   ./analysis/run.sh analyze track.mp3 --json
#   ./analysis/run.sh analyze --dir ~/Music/ --workers 8 --output results.json
#   ./analysis/run.sh benchmark --playlist "Test Set"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

if [ ! -d "$VENV_DIR" ]; then
    echo "Error: venv not found. Run ./analysis/setup.sh first." >&2
    exit 1
fi

exec "$VENV_DIR/bin/python" -m fourfour_analysis "$@"
