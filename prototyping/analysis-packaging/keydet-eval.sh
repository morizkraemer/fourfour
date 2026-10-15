#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
manifest="${MANIFEST:-$here/../../benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json}"
cache="${CACHE:-$here/../../benchmark/results/keydet-spec}"
exec "$here/target/release/keydet" eval "$manifest" "$cache" "$@"
