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

    // As Turu's app and Turu Mas's extension.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
    }
}

dependencies {
    // `CrashUploads`: each app brings Sentry's NDK part itself (Turu: `sentry-android`).
    implementation("io.sentry:sentry-android-core:${libs.versions.sentry.get()}")
    // Compat APIs in place of deprecated ones (insets, intents, package info): the version Turu's
    // app uses; Turu Mas's extension had 1.3.2 through Sentry.
    implementation(libs.core)
}
