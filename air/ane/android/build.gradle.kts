plugins {
    alias(libs.plugins.androidLibrary)
}

android {
    namespace = "com.muktiarafi.turu.air"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
    }

    // Java 8 class files: ADT dexes the extension's jar with the AIR SDK's own d8 (1.2.52 in
    // 51.5.1.1), which can't read Java 11's ("Compilation failed with an internal error",
    // `IllegalArgumentException` in its class reader). The verifier's module and Kotlin's
    // standard library, merged into the same jar, are Java 8 already.
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

// AIR's FlashRuntimeExtensions.jar: the runtime has it, so it stays out of the ANE.
val airHome: String = System.getenv("AIR_HOME") ?: error("set AIR_HOME to the AIR SDK")

// An ANE gives AIR one jar per platform, so the Kotlin standard library goes into it
// (air/ane/build.sh merges what `aneLibs` copies).
val bundled: Configuration by configurations.creating {
    isTransitive = false
}

dependencies {
    compileOnly(files("$airHome/lib/android/FlashRuntimeExtensions.jar"))
    bundled("org.jetbrains.kotlin:kotlin-stdlib:${libs.versions.kotlin.get()}")
}

tasks.register<Copy>("aneLibs") {
    from(bundled)
    into(layout.buildDirectory.dir("ane-libs"))
}
