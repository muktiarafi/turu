# R8 for the release APK: shrinks the libraries (Compose, Sentry, Kotlin's), keeps Turu's own code.

# Rust reaches Turu's Kotlin through JNI (src/java.rs, `bind_java_type!`), which R8 can't see:
# keep every class, method and field of Turu's own packages, with their names.
-keep class com.muktiarafi.turu.** { *; }

# rustls-platform-verifier's Kotlin half, called from Rust over JNI (its README, "Proguard").
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }

# Crash reports stay readable: Sentry gets the real class and method names and line numbers.
-dontobfuscate
-keepattributes SourceFile,LineNumberTable

# GameActivity's C++ half (built into libturu_android.so by android-activity) calls its Java half
# over JNI; the AAR's own rules don't keep all of it.
-keep class com.google.androidgamesdk.** { *; }
