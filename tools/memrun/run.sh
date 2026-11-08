#!/usr/bin/env bash
# Long headless memory run: logs in, runs a script, and logs Turu's `memory:` line every
# PERIOD seconds (TURU_PERF_LOG) until DURATION minutes pass or the script stops.
#
#   tools/memrun/run.sh [script] [minutes]
#
# The login comes from turu-args.txt (ARGS to use another file): `--user`, `--password` and
# `--server`, one argument per line as on Android. The password goes to Turu as TURU_PASSWORD,
# never on a command line. Its `--run-script` is ignored: the script is the first argument.
#
# Defaults: script = scripts/evil/nsod/0NecroticSwordOfDoom.ts, 120 minutes.
# Env: PERIOD (60), BUILD=0 to skip the cargo build, TURU_SYSTEM_ALLOC=1 for heap profilers
# (the memory line's commit then reads ~0; see docs/HEADLESS.md).
# Output: tools/memrun/out/<stamp>-<script>.log, then summarize.py prints the trend.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
script="${1:-$root/scripts/evil/nsod/0NecroticSwordOfDoom.ts}"
minutes="${2:-120}"
period="${PERIOD:-60}"
args_file="${ARGS:-$root/turu-args.txt}"
script="$(cd "$(dirname "$script")" && pwd)/$(basename "$script")"

# The value after a flag: the next line, or `--flag=value` on one line.
arg() {
  awk -v flag="$1" '
    /^[[:space:]]*(#|$)/ { next }
    { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
    take { print; exit }
    $0 == flag { take = 1; next }
    index($0, flag "=") == 1 { print substr($0, length(flag) + 2); exit }
  ' "$args_file"
}
[[ -r "$args_file" ]] || { echo "Can't read $args_file" >&2; exit 1; }
user="$(arg --user)"
server="$(arg --server)"
TURU_PASSWORD="${TURU_PASSWORD:-$(arg --password)}"
[[ -n "$user" && -n "$TURU_PASSWORD" ]] || { echo "No --user/--password in $args_file" >&2; exit 1; }
export TURU_PASSWORD

if [[ "${BUILD:-1}" != 0 ]]; then
  (cd "$root" && cargo build -p turu --no-default-features \
    --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen)
fi

out="$root/tools/memrun/out"
mkdir -p "$out"
log="$out/$(date +%Y%m%d-%H%M%S)-$(basename "$script" .ts).log"
echo "Logging to $log for $minutes min (memory every ${period}s), server ${server:-default}"

TURU_PERF_LOG="$period" TURU_PERF_LOG_ROWS="${TURU_PERF_LOG_ROWS:-5}" \
  "$root/target/debug/turu" --headless --user "$user" ${server:+--server "$server"} \
  --run-script "$script" --exit-on-script-stop >>"$log" 2>&1 &  # append: the footprint lines share the file
pid=$!
echo "turu pid $pid; Ctrl+C stops it early"
trap 'kill -INT $pid 2>/dev/null || true' INT TERM

# Also sample what macOS charges the process (phys_footprint), next to mimalloc's view.
end=$(( $(date +%s) + minutes * 60 ))
while kill -0 "$pid" 2>/dev/null && (( $(date +%s) < end )); do
  fp=$(footprint -p "$pid" 2>/dev/null | awk '/Footprint:/ {print $(NF-5), $(NF-4); exit}') || true
  [[ -n "${fp:-}" ]] && echo "memrun: $(date +%H:%M:%S) footprint $fp" >>"$log"
  sleep "$period" || true
done

if kill -0 "$pid" 2>/dev/null; then
  echo "Stopping turu"
  kill -INT "$pid" 2>/dev/null || true
  for _ in $(seq 20); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
  kill -9 "$pid" 2>/dev/null || true
fi
wait "$pid" 2>/dev/null || true

python3 "$root/tools/memrun/summarize.py" "$log"
