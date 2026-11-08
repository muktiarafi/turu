#!/usr/bin/env bash
# A farming run to measure: logs in, runs a script headless and, every PERIOD seconds, writes a
# `farmrun:` line (the control API's /status and what the process uses: CPU, memory, threads, open
# files, network and disk; tools/farmrun/sample.py) into the log, next to Turu's own `turu_perf`
# lines. Says at once when the character looks stuck or the run stops.
#
#   tools/farmrun/run.sh [script] [minutes]
#
# The login comes from turu-args.txt (ARGS to use another file), as in tools/memrun/run.sh: the
# password goes to Turu as TURU_PASSWORD, never on a command line. Its `--run-script` is ignored.
#
# Defaults: script = scripts/other/farmjoekits/0FarmerJoeDoAll.ts, 120 minutes (0: until the
# script stops). Env: PERIOD (60), STUCK_MIN (10: nothing moved for this long is "stuck"),
# API (127.0.0.1:9031), BUILD=0 to skip the cargo build, BIN (default target/debug/turu).
# Output: tools/farmrun/out/<stamp>-<script>.log, then summarize.py prints the report.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
script="${1:-$root/scripts/other/farmjoekits/0FarmerJoeDoAll.ts}"
minutes="${2:-120}"
period="${PERIOD:-60}"
stuck_min="${STUCK_MIN:-10}"
api="${API:-127.0.0.1:9031}"
bin="${BIN:-$root/target/debug/turu}"
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
# A token of this run's own, so nothing has to read the data folder's.
TURU_API_TOKEN="$(openssl rand -hex 32)"
export TURU_API_TOKEN

if [[ "${BUILD:-1}" != 0 ]]; then
  (cd "$root" && cargo build -p turu --no-default-features \
    --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen)
fi

out="$root/tools/farmrun/out"
mkdir -p "$out"
log="$out/$(date +%Y%m%d-%H%M%S)-$(basename "$script" .ts).log"
echo "Logging to $log ($([[ $minutes == 0 ]] && echo "until the script stops" || echo "$minutes min"), every ${period}s), server ${server:-default}, API $api"

TURU_PERF_LOG="$period" TURU_PERF_LOG_ROWS="${TURU_PERF_LOG_ROWS:-8}" \
  "$bin" --headless --user "$user" ${server:+--server "$server"} --api="$api" \
  --run-script "$script" --exit-on-script-stop >>"$log" 2>&1 &  # append: farmrun's lines share the file
pid=$!
echo "turu pid $pid; Ctrl+C stops it early"
trap 'kill -INT $pid 2>/dev/null || true' INT TERM

# One `farmrun:` line (sample.py): /status's progress and what the process uses (CPU, memory,
# threads, open files, network and disk bytes). Prints the progress signature for the stuck check.
sample() {
  local status
  status=$(curl -s -m 20 -H "Authorization: Bearer $TURU_API_TOKEN" "http://$api/status" || true)
  python3 "$root/tools/farmrun/sample.py" "$status" "$pid" "$log"
}

end=$(( minutes > 0 ? $(date +%s) + minutes * 60 : 0 ))
last_sig=""; since=$(date +%s); lines=$(wc -l <"$log"); warned=0
while kill -0 "$pid" 2>/dev/null && (( end == 0 || $(date +%s) < end )); do
  sleep "$period" || true
  kill -0 "$pid" 2>/dev/null || break
  sig=$(sample)
  # Any script log line counts as progress too (buying, travelling and talking kill nothing).
  now_lines=$(wc -l <"$log")
  if tail -n +"$((lines + 1))" "$log" | grep -q "turu::script"; then moved=1; else moved=0; fi
  lines=$now_lines
  if [[ "$sig" != "$last_sig" || $moved == 1 ]]; then
    last_sig=$sig; since=$(date +%s); warned=0
  fi
  idle=$(( ($(date +%s) - since) / 60 ))
  if (( idle >= stuck_min && idle / stuck_min > warned )); then
    warned=$(( idle / stuck_min ))
    echo "farmrun: $(date +%H:%M) STUCK? nothing moved for $idle min ($sig)" | tee -a "$log"
  fi
  case "$sig" in
    False\|*) echo "farmrun: $(date +%H:%M) not logged in ($sig)" ;;
    None\|*) echo "farmrun: $(date +%H:%M) /status didn't answer" ;;
  esac
done

if kill -0 "$pid" 2>/dev/null; then
  echo "Stopping turu"
  kill -INT "$pid" 2>/dev/null || true
  for _ in $(seq 20); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
  kill -9 "$pid" 2>/dev/null || true
fi
wait "$pid" 2>/dev/null || true

python3 "$root/tools/farmrun/summarize.py" "$log"
