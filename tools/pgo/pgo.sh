#!/usr/bin/env bash
# A speed profile (PGO) of Turu's own, recorded by its bot playing AQW, and the `dist` build that
# uses it: the flow Mugetsu's build-*.sh applies with Mugetsu's `aqw.profdata`, for Turu's binary.
#
#   tools/pgo/pgo.sh gen                  instrumented `dist` build: counts what runs
#   tools/pgo/pgo.sh record [script...]   play: log in, run each script for MINUTES, in MODES
#   tools/pgo/pgo.sh merge                the runs' counts -> out/turu-<target>.profdata
#   tools/pgo/pgo.sh use                  `dist` build with the profile -> target/<target>/dist/turu
#   tools/pgo/pgo.sh all [script...]      the four in turn
#
# The login comes from turu-args.txt (ARGS to use another file), as in tools/farmrun/run.sh: the
# password goes to Turu as TURU_PASSWORD, never on a command line. Use a spare account and a
# quiet server (SERVER overrides the file's): the run logs in and plays for real, and refuses
# Twilly, Artix and Alteon (nothing relogs on a crowded server).
#
# Defaults: script = scripts/other/farmjoekits/0FarmerJoeDoAll.ts. Several scripts cover more:
# combat, travel, shops, quests, a busy room.
# Env: MINUTES (20 a run), MODES ("window headless": a window draws the game, headless covers
# the server build; a headless run draws nothing, so it says nothing about drawing), API
# (127.0.0.1:9041), FEATURES (CLAUDE.md's feature set), TARGET (the host's triple).
# Needs llvm-profdata matching rustc's LLVM: `rustup component add llvm-tools`.
#
# Counts are written when the process exits cleanly: each run ends with `POST /script/stop`,
# which `--exit-on-script-stop` turns into a logout and an exit (in a window, the only tab closing
# itself quits the shell). A process killed (SIGINT, SIGKILL) or crashed (`panic = "abort"`)
# writes nothing. Re-record after large changes: a profile only covers the functions whose code
# it saw.
# Output: tools/pgo/out/ (raw counts, logs, the .profdata). Windows: not covered (bash).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
here="$root/tools/pgo"
out="$here/out"
raw="$out/raw"
minutes="${MINUTES:-20}"
modes="${MODES:-window headless}"
api="${API:-127.0.0.1:9041}"
features="${FEATURES:-software_video,external_video,lzma,jpegxr,fontconfig-dlopen}"
target="${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
profdata="$out/turu-$target.profdata"
gen_dir="$root/target/pgo-gen"
gen_bin="$gen_dir/$target/dist/turu"

# `--target` keeps RUSTFLAGS off build scripts and proc macros: only Turu's binary is measured.
# The C code (mimalloc, QuickJS, ...) goes through `cc`, which drops the profile flags cc-rs
# would pass on to a clang of another LLVM (see that file).
build() {
  local env_target="${target//-/_}"
  (cd "$root" && env "CC_$env_target=$here/cc" "CXX_$env_target=$here/c++" \
    cargo build --locked -p turu --profile dist --target "$target" \
    --no-default-features --features "$features")
}

gen() {
  rm -rf "$raw"
  mkdir -p "$raw"
  echo "pgo: instrumented build -> $gen_bin"
  CARGO_TARGET_DIR="$gen_dir" RUSTFLAGS="-Cprofile-generate=$raw" build
}

# The value after a flag in the args file: the next line, or `--flag=value` on one line.
arg() {
  awk -v flag="$1" '
    /^[[:space:]]*(#|$)/ { next }
    { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
    take { print; exit }
    $0 == flag { take = 1; next }
    index($0, flag "=") == 1 { print substr($0, length(flag) + 2); exit }
  ' "$args_file"
}

# One run: `$1` (window or headless) plays `$2` for MINUTES, then stops the script so Turu exits
# cleanly and writes its counts.
run_one() {
  local mode="$1" script="$2" log pid flags=()
  log="$out/$(date +%Y%m%d-%H%M%S)-$mode-$(basename "$script" .ts).log"
  [[ "$mode" == headless ]] && flags+=(--headless)
  echo "pgo: $mode, $(basename "$script"), $minutes min, server $server; log $log"
  LLVM_PROFILE_FILE="$raw/turu-%p-%m.profraw" \
    "$gen_bin" ${flags[@]+"${flags[@]}"} --user "$user" --server "$server" --api="$api" \
    --run-script "$script" --exit-on-script-stop >>"$log" 2>&1 &
  pid=$!
  trap 'stop "$pid"' INT TERM

  local end=$(($(date +%s) + minutes * 60))
  while kill -0 "$pid" 2>/dev/null && (($(date +%s) < end)); do
    sleep 5 || true
  done
  stop "$pid"
  trap - INT TERM
}

# Stops the run's script (Turu then logs out and exits); kills it only when it won't, and says
# that run's counts are lost.
stop() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null || { wait "$pid" 2>/dev/null || true; return; }
  curl -s -m 20 -X POST -H "Authorization: Bearer $TURU_API_TOKEN" "http://$api/script/stop" >/dev/null || true
  for _ in $(seq 60); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$pid" 2>/dev/null; then
    echo "pgo: turu ($pid) didn't exit after the script stopped; killed: this run's counts are lost" >&2
    kill -9 "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}

record() {
  [[ -x "$gen_bin" ]] || { echo "No instrumented build ($gen_bin): run \`$0 gen\` first" >&2; exit 1; }
  args_file="${ARGS:-$root/turu-args.txt}"
  [[ -r "$args_file" ]] || { echo "Can't read $args_file" >&2; exit 1; }
  user="$(arg --user)"
  server="${SERVER:-$(arg --server)}"
  TURU_PASSWORD="${TURU_PASSWORD:-$(arg --password)}"
  [[ -n "$user" && -n "$TURU_PASSWORD" ]] || { echo "No --user/--password in $args_file" >&2; exit 1; }
  case "$(tr '[:upper:]' '[:lower:]' <<<"$server")" in
    "" | twilly | artix | alteon)
      echo "pgo: name a quiet server (SERVER=..., or --server in $args_file), not '${server:-the default}'" >&2
      exit 1
      ;;
  esac
  export TURU_PASSWORD
  # A token of this run's own, so nothing has to read the data folder's.
  TURU_API_TOKEN="$(openssl rand -hex 32)"
  export TURU_API_TOKEN

  local scripts=("$@")
  ((${#scripts[@]})) || scripts=("$root/scripts/other/farmjoekits/0FarmerJoeDoAll.ts")
  mkdir -p "$raw"
  local script mode
  for script in "${scripts[@]}"; do
    script="$(cd "$(dirname "$script")" && pwd)/$(basename "$script")"
    for mode in $modes; do
      run_one "$mode" "$script"
    done
  done
  echo "pgo: $(find "$raw" -name '*.profraw' | wc -l | tr -d ' ') count file(s) in $raw"
}

merge() {
  local tool
  tool="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata"
  [[ -x "$tool" ]] || { echo "No $tool: rustup component add llvm-tools" >&2; exit 1; }
  local files=()
  while IFS= read -r file; do files+=("$file"); done < <(find "$raw" -name '*.profraw' -size +0)
  ((${#files[@]})) || { echo "No counts in $raw: run \`$0 record\` first" >&2; exit 1; }
  "$tool" merge -o "$profdata" "${files[@]}"
  echo "pgo: merged ${#files[@]} file(s) -> $profdata"
}

use() {
  [[ -r "$profdata" ]] || { echo "No $profdata: run \`$0 merge\` first" >&2; exit 1; }
  echo "pgo: build with $profdata -> $root/target/$target/dist/turu"
  RUSTFLAGS="-Cprofile-use=$profdata" build
}

case "${1:-}" in
  gen) gen ;;
  record) shift; record "$@" ;;
  merge) merge ;;
  use) use ;;
  all) shift; gen; record "$@"; merge; use ;;
  *) sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
