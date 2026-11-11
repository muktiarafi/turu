#!/bin/bash
# Turu AIR (docs/AIR_PROPOSAL.md): builds the APK (the host SWF, VibeSkua's skua.swf, Turu's
# native extension with the engine) and installs it on the connected device (adb), unless
# `--no-install`. The engine patches skua.swf, and the live client as skua loads it, at run time.
# Needs:
#   AIR_HOME      the AIR SDK (51.5+)
#   AIR_JAVA_HOME a JDK 17 (AIR's Gradle doesn't run on newer ones)
#   ANDROID_HOME  the Android SDK (default ~/Library/Android/sdk)
# Signing: AIR_KEYSTORE (a PKCS#12 file) and AIR_KEYSTORE_PASSWORD; without them, a development
# key made in target/air-app/ on the first run (an APK signed with it doesn't update one signed
# with another key). Output: target/air-app/TuruAir.apk.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${AIR_HOME:?set AIR_HOME to the AIR SDK}"
: "${AIR_JAVA_HOME:?set AIR_JAVA_HOME to a JDK 17}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
out=target/air-app
mkdir -p "$out"

# skua.swf as the Ruffle build bundles it; the icons.
cp bot/assets/skua.swf "$out/skua.swf"
rm -rf "$out/icons" && cp -R air/app/icons "$out/icons"

# The native extension, the host, and the APK (JDK 17 from here on).
air/ane/build.sh
mkdir -p "$out/ext" && cp target/air-ane/com.muktiarafi.turu.air.ane "$out/ext/"
export JAVA_HOME="$AIR_JAVA_HOME" PATH="$AIR_JAVA_HOME/bin:$PATH"
"$AIR_HOME/bin/amxmlc" -swf-version=51 -external-library-path+=target/air-ane/TuruAirNative.swc \
  -source-path+=air/app/src -output "$out/TuruAirHost.swf" air/app/src/TuruAirHost.as
# The Android resources' folder, as an absolute path (ADT's base for <resdir> isn't app.xml's).
sed "s|@RES@|$PWD/air/app/res|" air/app/app.xml > "$out/app.xml"
keystore=${AIR_KEYSTORE:-$out/dev.p12}
password=${AIR_KEYSTORE_PASSWORD:-turuairdev}
if [ -z "${AIR_KEYSTORE:-}" ] && [ ! -f "$keystore" ]; then
  "$AIR_HOME/bin/adt" -certificate -cn "Turu AIR (development)" 2048-RSA "$keystore" "$password"
fi
# The app's ID as app.xml has it (com.muktiarafi.turuair), not AIR's "air." + ID.
AIR_NOANDROIDFLAIR=true "$AIR_HOME/bin/adt" -package -target apk-captive-runtime -arch armv8 \
  -storetype pkcs12 -keystore "$keystore" -storepass "$password" \
  "$out/TuruAir.apk" "$out/app.xml" -extdir "$out/ext" -C "$out" TuruAirHost.swf skua.swf icons
ls -la "$out/TuruAir.apk"
if [ "${1:-}" != "--no-install" ]; then
  "$ANDROID_HOME/platform-tools/adb" install -r "$out/TuruAir.apk"
  # The notification (Android 13+); the service runs without it, hidden.
  "$ANDROID_HOME/platform-tools/adb" shell pm grant com.muktiarafi.turuair android.permission.POST_NOTIFICATIONS || true
fi
