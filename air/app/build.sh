#!/bin/bash
# Turu Mas (docs/TURU_MAS.md): builds the APK (the AIR host SWF, Turu's own turu.swf, Turu's
# native extension with the engine) and installs it on the connected device (adb), unless
# `--no-install`. The engine patches the live client, as turu.swf loads it, at run time.
# Needs:
#   AIR_HOME      the AIR SDK (51.5+)
#   AIR_JAVA_HOME a JDK 17 (AIR's Gradle doesn't run on newer ones)
#   ANDROID_HOME  the Android SDK (default ~/Library/Android/sdk)
# Version: TURU_VERSION (the release workflow sets it from the tag), else the root Cargo.toml's.
# Signing: MAS_KEYSTORE (a PKCS#12 file) and MAS_KEYSTORE_PASSWORD; without them, a development
# key made in target/air-app/ on the first run (an APK signed with it doesn't update one signed
# with another key). Output: target/air-app/TuruMas.apk.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${AIR_HOME:?set AIR_HOME to the AIR SDK}"
: "${AIR_JAVA_HOME:?set AIR_JAVA_HOME to a JDK 17}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
out=target/air-app
mkdir -p "$out"

# The version, and AIR's versionNumber for it. Android's version code is the one Turu's Gradle
# build gives the same version (android/app/build.gradle.kts, `versionCode`): 0.2.0-beta.3 is
# 200203, 0.2.0 200900. AIR makes the code major * 1,000,000 + minor * 1,000 + patch, so that code
# is written as three parts of up to 999.
version=${TURU_VERSION:-$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)}
if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-(alpha|beta|rc)\.?([0-9]*))? ]]; then
  case "${BASH_REMATCH[5]}" in alpha) stage=1 ;; beta) stage=2 ;; rc) stage=3 ;; *) stage=9 ;; esac
  pre=0
  [ -n "${BASH_REMATCH[5]}" ] && pre=$((10#${BASH_REMATCH[6]:-0}))
  code=$(((((10#${BASH_REMATCH[1]} * 100 + 10#${BASH_REMATCH[2]}) * 100 + 10#${BASH_REMATCH[3]}) * 1000) + stage * 100 + pre))
else
  code=1
fi
version_number="$((code / 1000000)).$((code / 1000 % 1000)).$((code % 1000))"
echo "Turu Mas $version (version code $code, AIR versionNumber $version_number)"

# Turu's host SWF, as the Ruffle build bundles it; the icons.
cp bot/assets/turu.swf "$out/turu.swf"
rm -rf "$out/icons" && cp -R air/app/icons "$out/icons"

# The native extension, the host, and the APK (JDK 17 from here on).
TURU_VERSION="$version" air/ane/build.sh
mkdir -p "$out/ext" && cp target/air-ane/com.muktiarafi.turu.air.ane "$out/ext/"
export JAVA_HOME="$AIR_JAVA_HOME" PATH="$AIR_JAVA_HOME/bin:$PATH"
"$AIR_HOME/bin/amxmlc" -swf-version=51 -external-library-path+=target/air-ane/TuruAirNative.swc \
  -source-path+=air/app/src -output "$out/TuruAirHost.swf" air/app/src/TuruAirHost.as
# The Android resources: ours, plus the network security config of rustls-platform-verifier's
# Kotlin part (plain HTTP only to the certificate authorities' hosts, for their revocation lists;
# app.xml points the app at it), as its own Android library would merge it. Its folder goes into
# app.xml as an absolute path (ADT's base for <resdir> isn't app.xml's).
rm -rf "$out/res" && cp -R air/app/res "$out/res" && mkdir -p "$out/res/xml"
cp android/vendor/rustls-platform-verifier/android/rustls-platform-verifier/src/main/res/xml/network_security_config.xml \
  "$out/res/xml/"
sed -e "s|@RES@|$PWD/$out/res|" -e "s|@VERSION_NUMBER@|$version_number|" -e "s|@VERSION@|$version|" \
  air/app/app.xml > "$out/app.xml"
keystore=${MAS_KEYSTORE:-$out/dev.p12}
password=${MAS_KEYSTORE_PASSWORD:-turumasdev}
if [ -z "${MAS_KEYSTORE:-}" ] && [ ! -f "$keystore" ]; then
  "$AIR_HOME/bin/adt" -certificate -cn "Turu Mas (development)" 2048-RSA "$keystore" "$password"
fi
# The app's ID as app.xml has it (com.muktiarafi.turumas), not AIR's "air." + ID.
AIR_NOANDROIDFLAIR=true "$AIR_HOME/bin/adt" -package -target apk-captive-runtime -arch armv8 \
  -storetype pkcs12 -keystore "$keystore" -storepass "$password" \
  "$out/TuruMas.apk" "$out/app.xml" -extdir "$out/ext" -C "$out" TuruAirHost.swf turu.swf icons
ls -la "$out/TuruMas.apk"
if [ "${1:-}" != "--no-install" ]; then
  "$ANDROID_HOME/platform-tools/adb" install -r "$out/TuruMas.apk"
  # The notification (Android 13+); the service runs without it, hidden.
  "$ANDROID_HOME/platform-tools/adb" shell pm grant com.muktiarafi.turumas android.permission.POST_NOTIFICATIONS || true
fi
