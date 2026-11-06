#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Mac night 3: every pending Mac check, headless, one summary (build/mac/night3-<date>/summary.md).

  scripts/mac/night3.sh --remote                  from the dev box: bundle the branches, ship them to the Mac, run, fetch the results
  scripts/mac/night3.sh --remote --collect        only wait for and fetch a run that is already going on the Mac
  scripts/mac/night3.sh [--only 1,3] [--dry-run]  on the Mac (what --remote starts there); --dry-run prints every command, runs nothing

Items: 1 ov-1028 build + scratch launch + first Settings open, 2 perf-night 10k, 3 r110 first result on a quiet machine,
       4 chunk 500 (CoreML, int8) against CoreML 1500, 5 cores-1 against cores-2 with the UI busy.
Options: --date YYYY-MM-DD  --rounds-first N (6)  --rounds-chunk N (3)  --rounds-cores N (3)  --n TRACKS (200)
         --ref-prefix P (refs/night3/ on the Mac)  --clean (drop worktrees and scratch homes at the end)
Env: MAC_HOST (mbp), NIGHT3_MIN_FREE_GB (35), NIGHT3_Q8_MODEL, FF_FIXTURE_ROOT, FOURFOUR_MODELS_DIR, CARGO_TARGET_DIR.
Never touches ~/Library/Application Support/Pioneer, ~/Library/Pioneer, rekordbox data or the installed app's library: every app
launch runs with HOME inside the night directory, and the protected paths are hashed before and after.
EOF
}

DRY=0
REMOTE=0
COLLECT=0
ONLY="1,2,3,4,5"
DATE_STAMP="$(date +%Y-%m-%d)"
ROUNDS_FIRST=6
ROUNDS_CHUNK=3
ROUNDS_CORES=3
N=200
REF_PREFIX=""
CLEAN=0
ORIGINAL_ARGS=("$@")
while [ $# -gt 0 ]; do
  case "$1" in
    --remote) REMOTE=1; shift ;;
    --collect) COLLECT=1; shift ;;
    --dry-run) DRY=1; shift ;;
    --only) ONLY="$2"; shift 2 ;;
    --date) DATE_STAMP="$2"; shift 2 ;;
    --rounds-first) ROUNDS_FIRST="$2"; shift 2 ;;
    --rounds-chunk) ROUNDS_CHUNK="$2"; shift 2 ;;
    --rounds-cores) ROUNDS_CORES="$2"; shift 2 ;;
    --n) N="$2"; shift 2 ;;
    --ref-prefix) REF_PREFIX="$2"; shift 2 ;;
    --clean) CLEAN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd -P)"
HELPERS="$ROOT/scripts/mac"
NIGHT="${NIGHT3_OUT:-$ROOT/build/mac/night3-$DATE_STAMP}"
RESULTS="$NIGHT/results"
HOST="${MAC_HOST:-mbp}"
REAL_HOME="$HOME"
LABELS="ov-1028 r110-first-result r98-mac-pair"
MIN_FREE_GB="${NIGHT3_MIN_FREE_GB:-35}"

say() {
  echo "[night3] $*" >&2
  [ "$DRY" = 1 ] || { mkdir -p "$NIGHT"; echo "[night3] $*" >> "$NIGHT/night.log"; }
}

die() {
  echo "[night3] REFUSED: $*" >&2
  exit 1
}

run() {
  if [ "$DRY" = 1 ]; then
    printf '+ '
    printf '%q ' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

run_in() {
  local dir="$1"
  shift
  if [ "$DRY" = 1 ]; then
    printf '+ (cd %q &&' "$dir"
    printf ' %q' "$@"
    printf ')\n'
    return 0
  fi
  (cd "$dir" && "$@")
}

run_in_to() {
  local dir="$1" log="$2"
  shift 2
  if [ "$DRY" = 1 ]; then
    printf '+ (cd %q &&' "$dir"
    printf ' %q' "$@"
    printf ') > %q 2>&1\n' "$log"
    return 0
  fi
  (cd "$dir" && "$@") > "$log" 2>&1
}

run_to() {
  local log="$1"
  shift
  if [ "$DRY" = 1 ]; then
    printf '+ '
    printf '%q ' "$@"
    printf '> %q 2>&1\n' "$log"
    return 0
  fi
  "$@" > "$log" 2>&1
}

want() {
  case ",$ONLY," in *",$1,"*) return 0 ;; *) return 1 ;; esac
}

ref_of() {
  echo "${REF_PREFIX}$1"
}

free_gb() {
  df -Pk "$1" | awk 'NR==2 {printf "%d", $4 / 1048576}'
}

remote_main() {
  [ "$(uname -s)" != Darwin ] || die "--remote runs on the dev box, not on the Mac"
  local branch bundle inputs remote_dir remote_night q8
  branch="$(git -C "$ROOT" rev-parse --abbrev-ref HEAD)"
  [ "$branch" != HEAD ] || die "check out a branch that carries scripts/mac/night3.sh"
  for label in $LABELS; do
    git -C "$ROOT" rev-parse --verify --quiet "refs/heads/$label" > /dev/null || die "branch $label is missing on this box"
  done
  q8="${NIGHT3_Q8_MODEL:-$HOME/.cache/r97-models/beat_this_q8sc.onnx}"
  [ -f "$q8" ] || die "int8 beat model not found at $q8 (NIGHT3_Q8_MODEL)"
  bundle="$NIGHT/night3.bundle"
  inputs='$HOME/dev/build/night3-inputs'
  remote_dir='$HOME/dev/build/fourfour-night3'
  remote_night="$remote_dir/build/mac/night3-$DATE_STAMP"
  mkdir -p "$NIGHT"
  say "runner branch $branch $(git -C "$ROOT" rev-parse --short HEAD); ov-1028 $(git -C "$ROOT" rev-parse --short ov-1028); r110-first-result $(git -C "$ROOT" rev-parse --short r110-first-result); r98-mac-pair $(git -C "$ROOT" rev-parse --short r98-mac-pair)"
  if [ "$COLLECT" = 0 ]; then
    run git -C "$ROOT" bundle create "$bundle" "$branch" $LABELS
    run ssh "$HOST" "mkdir -p $inputs"
    run rsync -t "$bundle" "$q8" "$HOST:dev/build/night3-inputs/"
    run ssh "$HOST" "set -e; export PATH=/opt/homebrew/bin:\$HOME/.cargo/bin:\$PATH; D=$remote_dir; [ -d \$D/.git ] || git init -q \$D; cd \$D; git fetch -q -f $inputs/night3.bundle 'refs/heads/*:refs/night3/*'; git checkout -q -f --detach 'refs/night3/$branch'; rm -rf $remote_night/DONE; mkdir -p $remote_night; NIGHT3_Q8_MODEL=$inputs/beat_this_q8sc.onnx nohup sh -c 'scripts/mac/night3.sh --date $DATE_STAMP --ref-prefix refs/night3/ --rounds-first $ROUNDS_FIRST --rounds-chunk $ROUNDS_CHUNK --rounds-cores $ROUNDS_CORES --n $N --only $ONLY; touch $remote_night/DONE' > $remote_night/stdout.log 2>&1 < /dev/null &"
  fi
  say "waiting for $HOST:$remote_night/DONE (progress: ssh $HOST tail -f $remote_night/night.log)"
  if [ "$DRY" = 1 ]; then
    run ssh "$HOST" "until [ -f $remote_night/DONE ]; do sleep 60; done"
  else
    until ssh -o ConnectTimeout=10 -o BatchMode=yes "$HOST" "[ -f $remote_night/DONE ]" 2> /dev/null; do sleep 60; done
  fi
  run rsync -a --exclude 'home-*' --exclude app --exclude bin --exclude models --exclude tmp --exclude runs --exclude night3-wt "$HOST:dev/build/fourfour-night3/build/mac/night3-$DATE_STAMP/" "$NIGHT/"
  [ "$DRY" = 1 ] || cat "$NIGHT/summary.md"
}

[ "$REMOTE" = 0 ] || { remote_main; exit 0; }

if [ "$DRY" = 0 ] && [ "$(uname -s)" != Darwin ]; then
  die "night3.sh measures the Mac (WKWebView, CoreML) and runs on macOS only; use --dry-run here or --remote from the dev box"
fi
if [ "$DRY" = 0 ] && [ -z "${NIGHT3_CAFFEINATED:-}" ] && command -v caffeinate > /dev/null; then
  NIGHT3_CAFFEINATED=1 exec caffeinate -i "$0" "${ORIGINAL_ARGS[@]}"
fi
if [ "$(uname -s)" = Darwin ]; then
  export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
fi

WT="$ROOT/build/mac/night3-wt"
TARGET="${CARGO_TARGET_DIR:-$ROOT/build/mac/night3-target}"
FIXTURE_ROOT="${FF_FIXTURE_ROOT:-$HOME/.cache/ff-fixture}"
FIX="$FIXTURE_ROOT/10k"
MODELS="$NIGHT/models"
APP="$NIGHT/app/fourfour.app"
Q8_MODEL="${NIGHT3_Q8_MODEL:-$HOME/.cache/r97-models/beat_this_q8sc.onnx}"
if [ "$DRY" = 1 ]; then
  CORES="<cores>"
  CORES_1="<cores-1>"
  CORES_2="<cores-2>"
else
  CORES="$(sysctl -n hw.logicalcpu)"
  CORES_1=$((CORES - 1))
  CORES_2=$((CORES - 2))
fi

lower() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}

refuse_unsafe_path() {
  local p="$1" l
  l="$(lower "$p")"
  [ -n "$p" ] || die "empty scratch path"
  [ "$p" != "$REAL_HOME" ] || die "scratch HOME is the real HOME"
  case "$REAL_HOME/" in "$p"/*) die "scratch path $p contains the real HOME" ;; esac
  case "$l" in
    *"/library/application support"*|*"/library/pioneer"*|*pioneer-usb*|*rekordbox*) die "scratch path $p points into app or Pioneer data" ;;
  esac
  case "$p" in
    "$NIGHT"/*) ;;
    *) die "scratch path $p is outside the night directory $NIGHT" ;;
  esac
}

scratch_home_make() {
  refuse_unsafe_path "$1"
  run mkdir -p "$1"
  run touch "$1/.night3-scratch"
}

scratch_home_check() {
  refuse_unsafe_path "$1"
  if [ "$DRY" = 0 ] && [ ! -f "$1/.night3-scratch" ]; then
    die "$1 was not created by night3 (no scratch marker)"
  fi
  say "scratch HOME ok: $1"
}

library_check() {
  local home="$1" config="$1/Library/Application Support/com.pioneer.test-ui/config.json" lib
  if [ "$DRY" = 1 ]; then
    say "library path check: config.json library_path must be inside $home"
    return 0
  fi
  lib="$(sed -n 's/.*"library_path":"\([^"]*\)".*/\1/p' "$config")"
  case "$lib" in
    "$home"/*) ;;
    *) die "library path '$lib' is not inside the scratch HOME $home" ;;
  esac
  case "$lib" in
    "$REAL_HOME/Library"*) die "library path '$lib' is the real library" ;;
  esac
}

PROTECTED_DIRS=(
  "$REAL_HOME/Library/Application Support/Pioneer"
  "$REAL_HOME/Library/Pioneer"
  "$REAL_HOME/Library/Application Support/com.pioneer.test-ui"
  "$REAL_HOME/Library/Application Support/rekordbox"
  "$REAL_HOME/Library/Application Support/rekordbox6"
)

file_stats() {
  if [ "$(uname -s)" = Darwin ]; then find "$1" -type f -exec stat -f '%N %z %m' {} +; else find "$1" -type f -exec stat -c '%n %s %Y' {} +; fi
}

protected_snapshot() {
  local out="$1" p
  if [ "$DRY" = 1 ]; then
    say "+ hash (find + stat, read-only) of: ${PROTECTED_DIRS[*]}"
    return 0
  fi
  : > "$out"
  for p in "${PROTECTED_DIRS[@]}"; do
    if [ -d "$p" ]; then
      printf '%s\t%s\n' "$p" "$(file_stats "$p" 2> /dev/null | LC_ALL=C sort | shasum -a 256 | cut -d' ' -f1)" >> "$out"
    else
      printf '%s\tabsent\n' "$p" >> "$out"
    fi
  done
}

preflight() {
  local free
  case "$NIGHT" in
    "$ROOT/build/"*) ;;
    *) die "night directory $NIGHT is not under $ROOT/build/" ;;
  esac
  refuse_unsafe_path "$NIGHT/perf/home-1"
  say "night directory: $NIGHT  runner: $(git -C "$ROOT" rev-parse --short HEAD)  host: $(uname -sm)  cores: $CORES"
  if [ "$DRY" = 1 ]; then
    say "+ preflight: tools (git cargo node npm python3 shasum), free disk >= ${MIN_FREE_GB} GB, fixture $FIX/library/library.db and long.txt, models $FOURFOUR_MODELS_DIR_RESOLVED, int8 model $Q8_MODEL"
    return 0
  fi
  local tool
  for tool in git cargo node npm python3 shasum awk; do
    command -v "$tool" > /dev/null || die "missing $tool"
  done
  cargo tauri --version > /dev/null 2>&1 || die "cargo tauri is missing (cargo install tauri-cli --version '^2')"
  mkdir -p "$NIGHT"
  free="$(free_gb "$NIGHT")"
  [ "$free" -ge "$MIN_FREE_GB" ] || die "only ${free} GB free, need ${MIN_FREE_GB} GB (cargo target and three worktrees)"
  [ -f "$FIX/library/library.db" ] && [ -f "$FIX/long.txt" ] || die "10k fixture missing at $FIX (scripts/perf.sh --size 10k generates it)"
  [ -f "$FOURFOUR_MODELS_DIR_RESOLVED/beat_this.onnx" ] || die "beat model missing in $FOURFOUR_MODELS_DIR_RESOLVED"
  [ -f "$Q8_MODEL" ] || die "int8 beat model missing at $Q8_MODEL (NIGHT3_Q8_MODEL)"
}

FOURFOUR_MODELS_DIR_RESOLVED="${FOURFOUR_MODELS_DIR:-$HOME/.cache/fourfour-models}"

write_meta() {
  [ "$DRY" = 0 ] || return 0
  {
    echo "Date $DATE_STAMP, host $(uname -sm), $(sw_vers -productVersion 2> /dev/null || uname -r), $CORES logical cores, load $(uptime | sed 's/.*load averages*: //')."
    echo "Runner $(git -C "$ROOT" rev-parse --short HEAD); ov-1028 $(git -C "$ROOT" rev-parse --short "$(ref_of ov-1028)"); r110-first-result $(git -C "$ROOT" rev-parse --short "$(ref_of r110-first-result)"); r98-mac-pair $(git -C "$ROOT" rev-parse --short "$(ref_of r98-mac-pair)")."
    echo "Free disk at start: $(free_gb "$NIGHT") GB."
  } > "$RESULTS/meta.txt"
}

prepare_worktree() {
  local label="$1" ref
  ref="$(ref_of "$label")"
  if [ "$DRY" = 0 ] && [ -e "$WT/$label/.git" ]; then
    git -C "$WT/$label" checkout -q -f --detach "$ref"
  else
    run git -C "$ROOT" worktree add -f --detach "$WT/$label" "$ref"
  fi
}

build_bench() {
  local label="$1" log="$NIGHT/build-bench-$1.log"
  prepare_worktree "$label"
  run_in_to "$WT/$label" "$log" env "CARGO_TARGET_DIR=$TARGET" cargo build --release --features pioneer-test-ui/dev-bins,pioneer-test-ui/coreml --bin bench_analysis
  run cp "$TARGET/release/bench_analysis" "$NIGHT/bin/bench-$label"
}

build_app() {
  local log="$NIGHT/build-app.log" src
  prepare_worktree ov-1028
  run_in_to "$WT/ov-1028/app" "$log" npm ci
  run_in_to "$WT/ov-1028/app" "$NIGHT/build-app-web.log" npm run build
  run_in_to "$WT/ov-1028/pioneer-test-ui" "$NIGHT/build-app-tauri.log" env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE -u APPLE_ID CI=true "CARGO_TARGET_DIR=$TARGET" cargo tauri build --features coreml --bundles app
  if [ "$DRY" = 1 ]; then
    run ditto "$TARGET/release/bundle/macos/<built>.app" "$APP"
  else
    src="$(ls -d "$TARGET"/release/bundle/macos/*.app | head -1)"
    [ -n "$src" ] || die "no .app in $TARGET/release/bundle/macos"
    ditto "$src" "$APP"
  fi
}

prepare_models() {
  run mkdir -p "$MODELS"
  run cp -R "$FOURFOUR_MODELS_DIR_RESOLVED/." "$MODELS/"
  run cp "$Q8_MODEL" "$MODELS/beat_this_q8sc.onnx"
}

wait_mark() {
  local log="$1" mark="$2" deadline=$((SECONDS + 240))
  while [ $SECONDS -lt $deadline ]; do
    grep -q "\"mark\":\"$mark\"" "$log" 2> /dev/null && return 0
    sleep 0.5
  done
  return 1
}

wait_quiet() {
  local i load
  if [ "$DRY" = 1 ]; then
    say "+ wait up to 300 s for the 1-minute load to fall below a third of $CORES cores"
    return 0
  fi
  for i in $(seq 1 60); do
    load="$(sysctl -n vm.loadavg | awk '{print $2}')"
    if awk -v l="$load" -v c="$CORES" 'BEGIN { exit !(l < c / 3) }'; then
      say "quiet before launch: load $load"
      return 0
    fi
    sleep 5
  done
  say "not quiet after 300 s (load $load), launching anyway: read the launch numbers as loaded"
}

app_bin() {
  local name
  name="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist" 2> /dev/null || echo pioneer-test-ui)"
  echo "$APP/Contents/MacOS/$name"
}

launch_app() {
  local name="$1" probe="$2" home="$NIGHT/home-$1" log="$NIGHT/app/$1.jsonl" data bin
  data="$home/Library/Application Support/com.pioneer.test-ui"
  scratch_home_make "$home"
  run mkdir -p "$data" "$NIGHT/app"
  run cp -R "$FIX/library" "$home/fixture-library"
  if [ "$DRY" = 1 ]; then
    say "+ write $data/config.json with library_path $home/fixture-library/library.db"
  else
    printf '{"library_path":"%s"}\n' "$home/fixture-library/library.db" > "$data/config.json"
  fi
  library_check "$home"
  scratch_home_check "$home"
  bin="$(app_bin)"
  if [ "$DRY" = 1 ]; then
    say "+ env HOME=$home FF_PERF_LOG=$log FF_PERF_PROBE='$probe' FOURFOUR_MODELS_DIR=$MODELS '$bin' &  (wait for probe_done, at most 240 s, then kill)"
    return 0
  fi
  : > "$log"
  env HOME="$home" FF_PERF_LOG="$log" FF_PERF_PROBE="$probe" FOURFOUR_MODELS_DIR="$MODELS" "$bin" > "$NIGHT/app/$name.out" 2>&1 &
  local pid=$!
  wait_mark "$log" probe_done || say "$name: no probe_done within 240 s"
  kill "$pid" 2> /dev/null || true
  wait "$pid" 2> /dev/null || true
  sleep 2
  rm -rf "$home"
}

item1() {
  [ -x "$(app_bin)" ] || [ "$DRY" = 1 ] || die "ov-1028 app was not built"
  wait_quiet
  launch_app settings '{"trackLoad":false,"scroll":false,"soakActions":["settings"],"soakMinutes":0.2,"soakPauseMs":1500,"soakSampleSecs":5}'
  run python3 -I "$HELPERS/night3-summary.py" item1 "$NIGHT/app/settings.jsonl" "$RESULTS/item1"
}

item2() {
  [ -x "$(app_bin)" ] || [ "$DRY" = 1 ] || die "ov-1028 app was not built"
  refuse_unsafe_path "$NIGHT/perf/home-1"
  wait_quiet
  run_in "$WT/ov-1028" env "CARGO_TARGET_DIR=$TARGET" "FF_PERF_NIGHT_OUT=$NIGHT/perf" "FOURFOUR_MODELS_DIR=$MODELS" "$WT/ov-1028/scripts/mac/perf-night.sh" --size 10k --app "$APP" --runs 3 --expand-runs 5
  run python3 -I "$HELPERS/night3-summary.py" item2 "$NIGHT/perf" "$RESULTS/item2"
}

item3() {
  local first="$NIGHT/first/first.jsonl" round order label bin
  run mkdir -p "$NIGHT/first" "$NIGHT/tmp"
  [ "$DRY" = 1 ] || : > "$first"
  for round in $(seq 1 "$ROUNDS_FIRST"); do
    if [ $((round % 2)) = 1 ]; then order="ov-1028 r110-first-result"; else order="r110-first-result ov-1028"; fi
    for label in $order; do
      bin="$NIGHT/bin/bench-$label"
      run python3 -I "$HELPERS/night3-first.py" --label "$label" --out "$first" --list "$FIX/long.txt" --limit 24 --wait-idle 0.8 --wait-max 60 --env FOURFOUR_BEAT_RUNTIME=rten --env "FOURFOUR_MODELS_DIR=$MODELS" --env "TMPDIR=$NIGHT/tmp" -- nice -n 5 "$bin"
    done
  done
  run python3 -I "$HELPERS/night3-summary.py" item3 "$first" "$RESULTS/item3"
}

TSV_HEADER=$'round\tlabel\ttracks\terrors\twall_s\ttracks_per_min\tfirst_result_s\tpeak_rss_mb\ttree_rss_mb\tcpu_s\texit'

bench_field() {
  printf '%s' "$1" | sed -n "s/.*[ ]$2=\([0-9.]*\).*/\1/p"
}

bench_run() {
  local tsv="$1" round="$2" label="$3" bin="$4" list="$5" limit="$6" dir status=0 line rss cpu
  dir="$NIGHT/runs/$label-$round"
  if [ "$DRY" = 1 ]; then
    printf '+ nice -n 10 /usr/bin/time -l -o %q env -u FOURFOUR_CHUNK_BUDGET_MB FOURFOUR_MODELS_DIR=%q TMPDIR=%q' "$dir/time.txt" "$MODELS" "$NIGHT/tmp"
    printf ' %q' ${BENCH_ENV[@]+"${BENCH_ENV[@]}"} "$bin" --list "$list" --limit "$limit" ${BENCH_ARGS[@]+"${BENCH_ARGS[@]}"}
    printf ' > %q 2> %q   # row %s round %s\n' "$dir/out.txt" "$dir/err.txt" "$label" "$round"
    return 0
  fi
  mkdir -p "$dir"
  nice -n 10 /usr/bin/time -l -o "$dir/time.txt" env -u FOURFOUR_CHUNK_BUDGET_MB FOURFOUR_MODELS_DIR="$MODELS" TMPDIR="$NIGHT/tmp" ${BENCH_ENV[@]+"${BENCH_ENV[@]}"} "$bin" --list "$list" --limit "$limit" ${BENCH_ARGS[@]+"${BENCH_ARGS[@]}"} > "$dir/out.txt" 2> "$dir/err.txt" || status=$?
  line="$(grep '^BENCH ' "$dir/out.txt" | tail -1 || true)"
  rss="$(awk '/maximum resident set size/ {printf "%.0f", $1 / 1048576}' "$dir/time.txt" 2> /dev/null)"
  cpu="$(awk '/ real / {printf "%.1f", $3 + $5}' "$dir/time.txt" 2> /dev/null)"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$round" "$label" "$(bench_field "$line" tracks)" "$(bench_field "$line" errors)" "$(bench_field "$line" wall)" "$(bench_field "$line" tracks_per_min)" "$(bench_field "$line" first_result)" "$rss" "$(bench_field "$line" peak_rss_tree)" "$cpu" "$status" >> "$tsv"
}

item4() {
  local tsv="$NIGHT/chunk/sweep.tsv" list="$NIGHT/chunk/tracks.txt" round
  run mkdir -p "$NIGHT/chunk" "$NIGHT/tmp"
  if [ "$DRY" = 1 ]; then
    say "+ { cat $FIX/long.txt; find $FIX/music -type f -size +2M | sort; } | awk (first $N unique) > $list"
  else
    { cat "$FIX/long.txt"; find "$FIX/music" -type f -size +2M | LC_ALL=C sort; } | awk -v n="$N" '!seen[$0]++ && c < n { print; c++ }' > "$list"
    [ "$(wc -l < "$list" | tr -d ' ')" -ge "$N" ] || die "fewer than $N real-length tracks in the fixture"
    printf '%s\n' "$TSV_HEADER" > "$tsv"
  fi
  for round in $(seq 1 "$ROUNDS_CHUNK"); do
    BENCH_ARGS=()
    BENCH_ENV=()
    bench_run "$tsv" "$round" a-coreml-1500 "$NIGHT/bin/bench-r98-mac-pair" "$list" "$N"
    BENCH_ENV=(FOURFOUR_BEAT_CHUNK=500)
    bench_run "$tsv" "$round" b-coreml-500 "$NIGHT/bin/bench-r98-mac-pair" "$list" "$N"
    BENCH_ENV=(FOURFOUR_BEAT_RUNTIME=rten FOURFOUR_BEAT_CHUNK=500 FOURFOUR_BEAT_MODEL=beat_this_q8sc.onnx)
    bench_run "$tsv" "$round" c-int8-rten-500 "$NIGHT/bin/bench-r98-mac-pair" "$list" "$N"
    BENCH_ENV=()
    bench_run "$tsv" "$round" d-ov1028-default "$NIGHT/bin/bench-ov-1028" "$list" "$N"
  done
  run python3 -I "$HELPERS/night3-summary.py" item4 "$tsv" "$RESULTS/item4"
}

item5() {
  local tsv="$NIGHT/cores/sweep.tsv" list="$NIGHT/cores/long24.txt" round
  run mkdir -p "$NIGHT/cores" "$NIGHT/tmp"
  if [ "$DRY" = 1 ]; then
    say "+ head -n 24 $FIX/long.txt > $list"
  else
    head -n 24 "$FIX/long.txt" > "$list"
    printf '%s\n' "$TSV_HEADER" > "$tsv"
  fi
  BENCH_ENV=()
  for round in $(seq 1 "$ROUNDS_CORES"); do
    BENCH_ARGS=(--threads "$CORES_1" --ui-busy 2000/6000)
    bench_run "$tsv" "$round" cores-1 "$NIGHT/bin/bench-ov-1028" "$list" 24
    BENCH_ARGS=(--threads "$CORES_2" --ui-busy 2000/6000)
    bench_run "$tsv" "$round" cores-2 "$NIGHT/bin/bench-ov-1028" "$list" 24
  done
  run python3 -I "$HELPERS/night3-summary.py" item5 "$tsv" "$RESULTS/item5"
}

run_item() {
  local n="$1" fn="item$1" rc
  want "$n" || { say "item $n skipped (--only)"; return 0; }
  say "item $n start"
  [ "$DRY" = 1 ] || rm -f "$RESULTS/item$n.status" "$RESULTS/item$n.md"
  set +e
  ( set -e; "$fn" )
  rc=$?
  set -e
  if [ "$rc" != 0 ] && [ "$DRY" = 0 ] && [ ! -s "$RESULTS/item$n.status" ]; then
    printf 'FAIL\tstep failed (exit %s), see night.log and the logs in the night directory\n' "$rc" > "$RESULTS/item$n.status"
  fi
  say "item $n done (exit $rc)"
}

build_all() {
  local label
  run mkdir -p "$NIGHT/bin" "$NIGHT/app" "$NIGHT/tmp"
  prepare_models
  if want 1 || want 2; then
    say "build: ov-1028 app (release, unsigned, coreml), kept in the night directory, /Applications untouched"
    set +e
    ( set -e; build_app )
    [ $? = 0 ] || say "app build failed (items 1 and 2 will fail)"
    set -e
  fi
  for label in $LABELS; do
    case "$label" in
      ov-1028) want 3 || want 4 || want 5 || continue ;;
      r110-first-result) want 3 || continue ;;
      r98-mac-pair) want 4 || continue ;;
    esac
    say "build: bench_analysis for $label (release, coreml)"
    set +e
    ( set -e; build_bench "$label" )
    [ $? = 0 ] || say "bench build for $label failed (items that need it will fail)"
    set -e
  done
}

cleanup() {
  [ "$CLEAN" = 1 ] || return 0
  local label
  for label in $LABELS; do
    run git -C "$ROOT" worktree remove --force "$WT/$label"
  done
  run rm -rf "$NIGHT/tmp" "$NIGHT/runs"
}

main() {
  local changed
  preflight
  run mkdir -p "$RESULTS"
  protected_snapshot "$RESULTS/protected-before.tsv"
  write_meta
  build_all
  run_item 1
  run_item 2
  run_item 3
  run_item 4
  run_item 5
  protected_snapshot "$RESULTS/protected-after.tsv"
  if [ "$DRY" = 1 ]; then
    say "+ compare protected-before.tsv with protected-after.tsv, then: python3 -I $HELPERS/night3-summary.py assemble $NIGHT"
    return 0
  fi
  if diff -q "$RESULTS/protected-before.tsv" "$RESULTS/protected-after.tsv" > /dev/null; then
    echo "unchanged ($(wc -l < "$RESULTS/protected-before.tsv" | tr -d ' ') paths hashed before and after)" > "$RESULTS/protected.txt"
    changed=0
  else
    echo "CHANGED, compare results/protected-before.tsv and protected-after.tsv (another app such as rekordbox may have written there)" > "$RESULTS/protected.txt"
    changed=1
  fi
  python3 -I "$HELPERS/night3-summary.py" assemble "$NIGHT"
  cleanup
  say "done: $NIGHT/summary.md"
  return "$changed"
}

main
