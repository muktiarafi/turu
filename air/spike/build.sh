#!/bin/bash
# Spike A0 (docs/AIR_PROPOSAL.md): builds an AIR APK that loads AQW's live client, patched by
# turu_air_patch, and installs it on the connected device (adb). Needs:
#   AIR_HOME      the AIR SDK (51.5+)
#   AIR_JAVA_HOME a JDK 17 (AIR's Gradle doesn't run on newer ones)
#   ANDROID_HOME  the Android SDK (default ~/Library/Android/sdk)
# Output in target/air-spike/. The signing key there is made on the first run, for this spike only.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${AIR_HOME:?set AIR_HOME to the AIR SDK}"
: "${AIR_JAVA_HOME:?set AIR_JAVA_HOME to a JDK 17}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
out=target/air-spike
mkdir -p "$out"

# The live client, patched.
sfile=$(curl -sf https://game.aq.com/game/api/data/gameversion | sed -E 's/.*"sFile":"([^"]+)".*/\1/')
echo "client: $sfile"
[ -f "$out/$sfile" ] || curl -sfo "$out/$sfile" "https://game.aq.com/game/gamefiles/$sfile"
cargo run -q --release -p turu_air_patch --example patch -- "$out/$sfile" "$out/Game-air.swf" | tail -2

# The native extension, the host, and the APK (JDK 17 from here on).
air/ane/build.sh
mkdir -p "$out/ext" && cp target/air-ane/com.muktiarafi.turu.air.ane "$out/ext/"
export JAVA_HOME="$AIR_JAVA_HOME" PATH="$AIR_JAVA_HOME/bin:$PATH"
"$AIR_HOME/bin/amxmlc" -swf-version=51 -external-library-path+=target/air-ane/TuruAirNative.swc \
  -output "$out/TuruAirHost.swf" air/spike/src/TuruAirHost.as
cp air/spike/app.xml "$out/app.xml"
[ -f "$out/spike.p12" ] || "$AIR_HOME/bin/adt" -certificate -cn TuruAirSpike 2048-RSA "$out/spike.p12" spikepass
"$AIR_HOME/bin/adt" -package -target apk-captive-runtime -arch armv8 \
  -storetype pkcs12 -keystore "$out/spike.p12" -storepass spikepass \
  "$out/TuruAirSpike.apk" "$out/app.xml" -extdir "$out/ext" -C "$out" TuruAirHost.swf Game-air.swf
ls -la "$out/TuruAirSpike.apk"
if [ "${1:-}" != "--no-install" ]; then
  "$ANDROID_HOME/platform-tools/adb" install -r "$out/TuruAirSpike.apk"
  # The notification (Android 13+); the service runs without it, hidden.
  "$ANDROID_HOME/platform-tools/adb" shell pm grant air.com.muktiarafi.turu.airspike android.permission.POST_NOTIFICATIONS || true
fi
