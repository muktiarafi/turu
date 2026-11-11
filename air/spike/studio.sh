#!/bin/bash
# Spike A0, "One APK or two?" (docs/AIR_PROPOSAL.md): the spike as an app whose launcher is
# Turu's own activity (TuruLauncher, in the extension) instead of AIR's AppEntry, and whose ID
# has no "air." prefix. ADT refuses a LAUNCHER category in app.xml's manifest additions, so this
# exports an Android Studio project (adt -target android-studio), moves the category in its
# manifest, and builds it with the Gradle the export asks for (8.13, from Gradle's cache, where
# ADT's own builds put it). Run air/spike/build.sh --no-install first; same variables.
# Output: target/air-spike/studio/app/build/outputs/apk/debug/app-debug.apk (debug-signed).
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${AIR_HOME:?set AIR_HOME to the AIR SDK}"
: "${AIR_JAVA_HOME:?set AIR_JAVA_HOME to a JDK 17}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export JAVA_HOME="$AIR_JAVA_HOME" PATH="$AIR_JAVA_HOME/bin:$PATH" AIR_NOANDROIDFLAIR=true
out=target/air-spike
[ -f "$out/TuruAirHost.swf" ] || { echo "run air/spike/build.sh --no-install first"; exit 1; }

# TuruLauncher with MAIN only: ADT allows that.
python3 - "$out/app.xml" <<'EOF'
import sys
p = sys.argv[1]
s = open("air/spike/app.xml").read()
s = s.replace("<application>\n", """<application>
  <activity android:name="com.muktiarafi.turu.air.TuruLauncher" android:exported="true" android:theme="@android:style/Theme.NoDisplay">
    <intent-filter><action android:name="android.intent.action.MAIN"/></intent-filter>
  </activity>
""", 1)
open(p, "w").write(s)
EOF
rm -rf "$out/studio"
"$AIR_HOME/bin/adt" -package -target android-studio "$out/studio" "$out/app.xml" \
  -extdir "$out/ext" -C "$out" TuruAirHost.swf skua.swf -platformsdk "$ANDROID_HOME"

# The LAUNCHER category moves from AIR's activity to TuruLauncher.
python3 - "$out/studio/app/src/main/AndroidManifest.xml" <<'EOF'
import re, sys
p = sys.argv[1]
s = open(p).read()
launcher = re.compile(r'<category android:name="android.intent.category.LAUNCHER"\s*>\s*</category>\s*')
s, n = launcher.subn("", s, count=1)
assert n == 1, "AIR's LAUNCHER category not found"
i = s.index('android:name="com.muktiarafi.turu.air.TuruLauncher"')
j = s.index("</intent-filter>", i)
s = s[:j] + '<category android:name="android.intent.category.LAUNCHER"/>\n' + s[j:]
open(p, "w").write(s)
EOF

gradle=$(ls -d "$HOME"/.gradle/wrapper/dists/gradle-8.13-bin/*/gradle-8.13/bin/gradle 2>/dev/null | head -1)
[ -n "$gradle" ] || { echo "Gradle 8.13 not in Gradle's cache: run air/spike/build.sh once"; exit 1; }
(cd "$out/studio" && "$gradle" -q assembleDebug)
ls -la "$out/studio/app/build/outputs/apk/debug/app-debug.apk"
