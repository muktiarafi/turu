// The Kotlin both Android apps share: Turu's app (`android/app`) and Turu Mas's extension
// (`air/ane/android`, which includes this module by path and merges its classes into the ANE's
// jar: air/ane/build.sh). Crash records and uploads, the "Send crash reports?" dialog, and the
// native text field above the keyboard (each app places it its own way).
plugins {
    alias(libs.plugins.androidLibrary)
}

android {
    namespace = "com.muktiarafi.turu.shared"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
    }

    // Java 8 class files: the AIR SDK's own d8 dexes Turu Mas's extension jar and can't read
    // Java 11's (air/ane/android/build.gradle.kts says more). Turu's app reads them as well.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8)
    }
}

dependencies {
    // `CrashUploads`: each app brings Sentry's NDK part itself (Turu: `sentry-android`).
    implementation("io.sentry:sentry-android-core:${libs.versions.sentry.get()}")
}
