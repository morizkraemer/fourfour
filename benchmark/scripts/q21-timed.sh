#!/usr/bin/env bash
# usage: q21-timed.sh <prefix> <command...>   writes <prefix>.out, <prefix>.err, <prefix>.time (GNU time -v)
set -u
prefix="$1"
shift
/usr/bin/time -v -o "$prefix.time" "$@" > "$prefix.out" 2> "$prefix.err"
grep -E "Elapsed|Maximum resident|User time|System time" "$prefix.time"
grep -E "SUMMARY|workers" "$prefix.err" || true
