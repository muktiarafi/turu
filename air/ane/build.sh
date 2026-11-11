#!/bin/bash
# Builds Turu's AIR native extension (target/air-ane/com.muktiarafi.turu.air.ane and its .swc).
# Needs AIR_HOME (AIR SDK 51.5+), AIR_JAVA_HOME (a JDK 17, for AIR's tools), ANDROID_HOME
# (Android SDK with the NDK) and cargo-ndk. The Kotlin side builds with Turu's Gradle wrapper and
# the default JDK; the Rust side (crate turu_air) with cargo-ndk.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${AIR_HOME:?set AIR_HOME to the AIR SDK}"
: "${AIR_JAVA_HOME:?set AIR_JAVA_HOME to a JDK 17}"
export AIR_HOME ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
out=target/air-ane
rm -rf "$out" && mkdir -p "$out/classes" "$out/Android-ARM64" "$out/default"

# Kotlin: the library's classes plus the Kotlin standard library, in the one jar AIR takes.
android/gradlew -q -p air/ane/android assembleRelease aneLibs
unzip -qo -p air/ane/android/build/outputs/aar/*-release.aar classes.jar > "$out/library-classes.jar"
for jar in "$out/library-classes.jar" air/ane/android/build/ane-libs/*.jar; do
  unzip -qo "$jar" -d "$out/classes" -x 'META-INF/MANIFEST.MF' 'META-INF/versions/*' 'module-info.class'
done
"$AIR_JAVA_HOME/bin/jar" cf "$out/Android-ARM64/turu-air.jar" -C "$out/classes" .

# Rust: libturu_air.so, which AIR packs from the platform's libs/<abi>.
cargo ndk -t arm64-v8a -o "$out/jni" build -q --release -p turu_air
mkdir -p "$out/Android-ARM64/libs/arm64-v8a"
cp "$out/jni/arm64-v8a/libturu_air.so" "$out/Android-ARM64/libs/arm64-v8a/"

# AS3, and the ANE (JDK 17 from here on).
export JAVA_HOME="$AIR_JAVA_HOME" PATH="$AIR_JAVA_HOME/bin:$PATH"
"$AIR_HOME/bin/acompc" -source-path air/ane/as3/src \
  -include-classes com.muktiarafi.turu.air.TuruAirNative -swf-version=51 \
  -output "$out/TuruAirNative.swc"
unzip -qo -p "$out/TuruAirNative.swc" library.swf > "$out/Android-ARM64/library.swf"
cp "$out/Android-ARM64/library.swf" "$out/default/library.swf"

"$AIR_HOME/bin/adt" -package -target ane "$out/com.muktiarafi.turu.air.ane" air/ane/extension.xml \
  -swc "$out/TuruAirNative.swc" \
  -platform Android-ARM64 -C "$out/Android-ARM64" . \
  -platform default -C "$out/default" .
ls -la "$out/com.muktiarafi.turu.air.ane"
