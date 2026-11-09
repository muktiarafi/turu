plugins {
    alias(libs.plugins.androidLibrary)
}

android {
    namespace = "com.muktiarafi.turu.air"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
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
