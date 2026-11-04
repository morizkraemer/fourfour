#!/usr/bin/env bash
# usage: r111-load-state.sh pairs <library dir> <pairs> <runs> <label=bench_startup binary>...
#        r111-load-state.sh identity <library dir> <label=bench_startup binary>...
# pairs: runs `bench_startup rep` (warm, in-process load_state repeats; medians and min-max) for each binary in turn, <pairs> times.
# identity: sha256 of the load_state response of each binary on a copy of the library dir (library.db and track-labels.json, anlz linked).
# The library dir is opened read-write by bench_startup: pass a copy, not a fixture.
set -eu
mode="$1"
shift
case "$mode" in
pairs)
    dir="$1" pairs="$2" runs="$3"
    shift 3
    for ((i = 0; i < pairs; i++)); do
        for spec in "$@"; do
            printf '%s ' "${spec%%=*}"
            nice -n 10 "${spec#*=}" rep "$dir" "$runs"
        done
    done
    ;;
identity)
    dir="$1"
    shift
    out="$(mktemp -d "${TMPDIR:-$HOME/.cache}/r111-identity.XXXXXX")"
    for spec in "$@"; do
        nice -n 10 "${spec#*=}" dump "$dir" "$out/${spec%%=*}.json"
        printf '%s ' "${spec%%=*}"
        sha256sum "$out/${spec%%=*}.json" | cut -d' ' -f1
    done
    rm -rf "$out"
    ;;
*)
    echo "usage: r111-load-state.sh pairs|identity ..." >&2
    exit 2
    ;;
esac
