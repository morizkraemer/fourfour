#!/usr/bin/env bash
set -euo pipefail

MINUTES="${1:-45}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
APP="${FF_SOAK_APP:-$TARGET/release/pioneer-test-ui}"
FIXTURE_LIBRARY="${FF_SOAK_LIBRARY:-$HOME/.cache/ff-fixture/10k/library}"
MODELS="${FF_SOAK_MODELS:-$HOME/.cache/fourfour-models}"
OUT="${FF_SOAK_OUT_DIR:-${TMPDIR:-/tmp}/ff-soak-real-out}"
SAMPLE_SECS="${FF_SOAK_SAMPLE_SECS:-60}"
ACTIONS="${FF_SOAK_ACTIONS:-}"
say() { echo "[soak-real] $*" >&2; }

[ -x "$APP" ] || { say "app binary $APP missing (cargo build --release -p pioneer-test-ui --features tauri/custom-protocol)"; exit 1; }
[ -f "$FIXTURE_LIBRARY/library.db" ] || { say "no library.db in $FIXTURE_LIBRARY"; exit 1; }
command -v xvfb-run >/dev/null || { say "xvfb-run not found"; exit 1; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/ff-soak-real.XXXXXX")"
mkdir -p "$OUT" "$WORK/data/com.pioneer.test-ui" "$WORK/config" "$WORK/cache"
LOG="$OUT/probe.jsonl"
CSV="$OUT/rss.csv"
: >"$LOG"
leader=""
cleanup() {
  [ -n "$leader" ] && kill -TERM -- "-$leader" 2>/dev/null || true
  sleep 1
  [ -n "$leader" ] && kill -KILL -- "-$leader" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

say "copying the library to $WORK"
cp -r "$FIXTURE_LIBRARY"/. "$WORK/data/com.pioneer.test-ui/"

if [ "${FF_SOAK_AUTO_ANALYZE:-on}" = "off" ]; then
  echo '{"auto_analyze_new_tracks": false}' >"$WORK/data/com.pioneer.test-ui/config.json"
fi

IDS="$(seq -s, 7 250 10000)"
ACTIONS_JSON=""
if [ "$ACTIONS" = "none" ]; then ACTIONS_JSON='"soakActions":[],"soakPauseMs":1000,'
elif [ -n "$ACTIONS" ]; then ACTIONS_JSON="\"soakActions\":[\"${ACTIONS//,/\",\"}\"],"; fi
PROBE="{${ACTIONS_JSON}\"soakMinutes\":$MINUTES,\"soakSampleSecs\":$SAMPLE_SECS,\"soakAnalysisIds\":[$IDS]}"

export HOME="$WORK" XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config" XDG_CACHE_HOME="$WORK/cache"
export FF_PERF_LOG="$LOG" FF_PERF_PROBE="$PROBE" FOURFOUR_MODELS_DIR="$MODELS"
export WEBKIT_DISABLE_COMPOSITING_MODE="${WEBKIT_DISABLE_COMPOSITING_MODE:-1}"
export NO_AT_BRIDGE=1

setsid xvfb-run -a -s "-screen 0 1600x900x24" bash -c 'echo $$ >"$0"; exec "$1"' "$WORK/app.pid" "$APP" >"$OUT/app.stderr" 2>&1 &
leader=$!
for _ in $(seq 60); do [ -s "$WORK/app.pid" ] && break; sleep 1; done
APP_PID="$(cat "$WORK/app.pid")"
say "app pid $APP_PID"

kb() { awk -v k="$2" '$1==k {print $2}' "$1" 2>/dev/null || true; }
sum_pids() {
  local rss=0 pss=0 n=0 pid r p
  for pid in "$@"; do
    r="$(kb "/proc/$pid/status" VmRSS:)"
    p="$(kb "/proc/$pid/smaps_rollup" Pss:)"
    rss=$((rss + ${r:-0}))
    pss=$((pss + ${p:-0}))
    n=$((n + 1))
  done
  echo "$rss $pss $n"
}
children_named() { local pid; for pid in $(pgrep -P "$APP_PID" 2>/dev/null); do [ "$(cat "/proc/$pid/comm" 2>/dev/null)" = "$1" ] && echo "$pid"; done; true; }

analysed_tracks() {
  python3 -I -c 'import sqlite3,sys; print(sqlite3.connect("file:"+sys.argv[1]+"?mode=ro", uri=True, timeout=5).execute("select count(*) from analyses").fetchone()[0])' "$WORK/data/com.pioneer.test-ui/library.db" 2>/dev/null || echo -1
}
dump_spike() {
  { echo "== $1 s"; grep -E "^(Rss|Pss|Anonymous|Private_Dirty|Swap):" "/proc/$APP_PID/smaps_rollup"
    ps -L -o tid,pcpu,comm -p "$APP_PID" | sort -k2 -nr | head -12
    ps -L -o comm= -p "$APP_PID" | sort | uniq -c | sort -rn | head -8; } >>"$OUT/spikes.txt" 2>&1
}
: >"$OUT/spikes.txt"
echo "elapsed_s,app_rss_kb,app_pss_kb,app_threads,app_fds,web_rss_kb,web_pss_kb,web_procs,net_rss_kb,net_pss_kb,analysed_tracks" >"$CSV"
start=$(date +%s)
deadline=$((start + MINUTES * 60 + 900))
while kill -0 "$APP_PID" 2>/dev/null; do
  now=$(date +%s)
  elapsed=$((now - start))
  read -r a_rss a_pss _ <<<"$(sum_pids "$APP_PID")"
  threads="$(awk '$1=="Threads:" {print $2}' "/proc/$APP_PID/status" 2>/dev/null || echo 0)"
  fds="$(ls "/proc/$APP_PID/fd" 2>/dev/null | wc -l)"
  read -r w_rss w_pss w_n <<<"$(sum_pids $(children_named WebKitWebProces))"
  read -r n_rss n_pss _ <<<"$(sum_pids $(children_named WebKitNetworkPr))"
  echo "$elapsed,$a_rss,$a_pss,$threads,$fds,$w_rss,$w_pss,$w_n,$n_rss,$n_pss,$(analysed_tracks)" >>"$CSV"
  [ "$a_rss" -gt 3000000 ] && dump_spike "$elapsed"
  if grep -q '"mark": *"probe_done"' "$LOG"; then say "probe done"; break; fi
  [ "$now" -ge "$deadline" ] && { say "deadline reached without probe_done"; break; }
  sleep 60
done
say "csv: $CSV, probe log: $LOG, app stderr: $OUT/app.stderr"
