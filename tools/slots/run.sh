#!/bin/bash
# M1 spike on a phone (docs/TAB_PROCESSES_PROPOSAL.md, "M1 results"): memory of two accounts in
# one process (tabs) against two slot processes, switching, a slot killed, the shell killed, the
# backoff of repeated kills and the file-lock probe. Needs the spike's debug APK installed
# (`cd android && ./gradlew assembleDebug`, branch spike/m1-slots), adb with one phone attached,
# and two accounts: put each one's flags in the data folder first, one per line, as turu-args.txt:
#
#   turu-args-c1.txt   --user <account 1>  --password <...>  --server <a quiet server>
#   turu-args-c2.txt   --user <account 2>  --password <...>  --server <the same>
#
# (never Twilly, Artix or Alteon: the kill tests make relogins). The tabs baseline uses
# turu-args.txt (account 1) and opens account 2 in a second tab by hand when asked.
#
# Usage: tools/slots/run.sh [out-dir]   (prints and writes results.txt)
set -u
OUT=${1:-tools/slots/out/$(date +%Y%m%d-%H%M%S)}
mkdir -p "$OUT"
PKG=com.muktiarafi.turu
D=/sdcard/Android/data/$PKG/files
SETTLE=${SETTLE:-120}
say() { echo "$*" | tee -a "$OUT/results.txt"; }
pid() { adb shell pidof "$1" | tr -d '\r'; }
mem() {
  for name in $PKG $PKG:c1 $PKG:c2; do
    local p; p=$(pid $name); [ -z "$p" ] && continue
    local line; line=$(adb shell dumpsys meminfo "$p" | grep -m1 "TOTAL PSS")
    local pss rss swap
    pss=$(echo "$line" | sed -E 's/.*TOTAL PSS: *([0-9]+).*/\1/')
    rss=$(echo "$line" | sed -E 's/.*TOTAL RSS: *([0-9]+).*/\1/')
    swap=$(echo "$line" | sed -E 's/.*TOTAL SWAP[^:]*: *([0-9]+).*/\1/')
    say "  $1 $name: PSS $((pss / 1024)) MB, swap $((swap / 1024)) MB, RSS $((rss / 1024)) MB"
  done
}
shell() { adb shell am start -n $PKG/.TuruActivity "$@" >/dev/null 2>&1; }

for f in turu-args.txt turu-args-c1.txt turu-args-c2.txt; do
  adb shell "test -f $D/$f" || { echo "Missing $D/$f (see the top of this script)"; exit 1; }
done
say "Phone: $(adb shell getprop ro.product.manufacturer | tr -d '\r') $(adb shell getprop ro.product.model | tr -d '\r'), Android $(adb shell getprop ro.build.version.release | tr -d '\r'), $(adb shell cat /proc/meminfo | awk '/MemTotal/ {print int($2/1024)}') MB"
adb shell "cp $D/android-tabs.json $D/android-tabs.json.bak 2>/dev/null; echo '[]' > $D/android-tabs.json; rm -f $D/turu-slots.txt"

say "== Tabs (one process)"
adb shell am force-stop $PKG; shell; sleep "$SETTLE"
say " one account, in game, shown:"; mem tabs-1
read -r -p "Open Army > New Tab, log account 2 in on it, go back to tab 1, then press Enter... "
sleep 60; say " two accounts in game, tab 1 shown:"; mem tabs-2

say "== Slots (a process each)"
adb shell am force-stop $PKG; adb shell "echo on > $D/turu-slots.txt"; shell; sleep "$SETTLE"
# Both slots drawn once before the first reading, as both tabs were in the tabs run.
adb logcat -c
for s in 2 1 2 1; do shell --ei slot $s; sleep 4; done
say " switches:"; adb logcat -d -s turu-slots | grep "switched to" | sed 's/.*Shell: /   /' | tee -a "$OUT/results.txt"
say "  drawing errors in the slots: $(adb logcat -d | grep -c "Couldn't set up the window\|Couldn't draw in the new window")"
sleep 60; say " slot 1 shown, both in game:"; mem slots-1
shell --ei slot 2; sleep 30; say " slot 2 shown:"; mem slots-2
adb shell input keyevent HOME; sleep 60; say " app in the background:"; mem slots-bg

say "== Slot 1 killed (slot 2 should keep its process)"
p1=$(pid $PKG:c1); p2=$(pid $PKG:c2); t0=$(date +%s); adb logcat -c
adb shell run-as $PKG kill -9 "$p1"
for _ in $(seq 1 60); do n=$(pid $PKG:c1); [ -n "$n" ] && break; sleep 1; done
say "  back after $(($(date +%s) - t0)) s; slot 2 $([ "$(pid $PKG:c2)" = "$p2" ] && echo kept its process || echo changed)"
sleep 60; say "  slot 1 relogin lines after the kill: $(adb logcat -d -s turu | grep " $n " | grep -c -E "Relogin|Logging in")"

say "== Shell killed (both slots should keep running and let go of the surface)"
shell; sleep 10; p0=$(pid $PKG); p1=$(pid $PKG:c1); p2=$(pid $PKG:c2); adb logcat -c
adb shell run-as $PKG kill -9 "$p0"; sleep 10
say "  slots kept: $([ "$(pid $PKG:c1)" = "$p1" ] && [ "$(pid $PKG:c2)" = "$p2" ] && echo yes || echo no)"
adb logcat -d -s turu-slots | grep "shell died" | sed 's/^.\{31\}/   /' | tee -a "$OUT/results.txt"
shell; sleep 10; adb exec-out screencap -p >"$OUT/after-shell-kill.png"; say "  screenshot: $OUT/after-shell-kill.png"

say "== Backoff of repeated kills (slot 2)"
for k in 1 2 3; do
  p=$(pid $PKG:c2); adb logcat -c; t0=$(date +%s); adb shell run-as $PKG kill -9 "$p"
  for _ in $(seq 1 120); do n=$(pid $PKG:c2); [ -n "$n" ] && break; sleep 1; done
  say "  kill $k: back after $(($(date +%s) - t0)) s"; sleep 5
done

say "== File locks between the slots (0 held, 1 busy, 2 let go)"
adb logcat -c
probe() { shell --ei lock "$1" --ei lockAction "$2"; sleep 2; }
probe 1 1; probe 2 1; probe 1 2; probe 2 1; probe 2 2
adb logcat -d -s turu-slots | grep "lock probe" | sed 's/.*Shell: /  /' | tee -a "$OUT/results.txt"

say "== Low memory: open other heavy apps (games, the camera, a browser with tabs) until Android kills one Turu process, then press Enter"
read -r -p "... "
adb logcat -d | grep -E "lowmemorykiller|lmkd.*muktiarafi|has died.*muktiarafi" | tail -10 | tee -a "$OUT/results.txt"
mem after-pressure

adb shell am force-stop $PKG
adb shell "rm -f $D/turu-slots.txt; mv $D/android-tabs.json.bak $D/android-tabs.json 2>/dev/null"
say "Done: $OUT/results.txt (the slot args files are left in the data folder; delete them when finished)"
