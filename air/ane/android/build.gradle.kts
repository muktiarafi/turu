plugins {
    alias(libs.plugins.androidLibrary)
}

android {
    namespace = "com.muktiarafi.turu.air"
    compileSdk = 36

    defaultConfig {
        minSdk = 26
        // Crash reports to Sentry (`CrashUploads.kt`), sent only once the user said yes: the DSN
        // from `TURU_MAS_SENTRY_DSN` when building (the release workflow passes its secret);
        // empty for none.
        buildConfigField("String", "SENTRY_DSN", "\"${System.getenv("TURU_MAS_SENTRY_DSN")?.trim() ?: ""}\"")
    }

    buildFeatures {
        buildConfig = true
    }

    // Java 8 class files: ADT dexes the extension's jar with the AIR SDK's own d8 (1.2.52 in
    // 51.5.1.1), which can't read Java 11's ("Compilation failed with an internal error",
    // `IllegalArgumentException` in its class reader). The verifier's module, Kotlin's standard
    // library and Sentry's, merged into the same jar, are Java 8 already.
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

dependencies {
    compileOnly(files("$airHome/lib/android/FlashRuntimeExtensions.jar"))
    // Crash reports as Turu's Android app sends them (`io.sentry:sentry-android` there): Java and
    // native crashes (the NDK part), ANRs and tombstones. Not its session replay, which Turu
    // doesn't use either.
    implementation("io.sentry:sentry-android-core:${libs.versions.sentry.get()}")
    implementation("io.sentry:sentry-android-ndk:${libs.versions.sentry.get()}")
}

// An ANE gives AIR one jar per platform and its native libraries, so what the library needs at
// run time goes into it: air/ane/build.sh merges the jars `aneLibs` writes (the Kotlin standard
// library, Sentry and the AndroidX parts it uses; each library's classes, numbered, as AARs all
// call theirs classes.jar) and packs `jni/arm64-v8a/*.so` (Sentry's native handler) with
// libturu_air.so.
tasks.register("aneLibs") {
    val runtime = configurations.getByName("releaseRuntimeClasspath")
    val artifactType = Attribute.of("artifactType", String::class.java)
    val classes = runtime.incoming.artifactView {
        attributes { attribute(artifactType, "android-classes-jar") }
    }.files
    val jni = runtime.incoming.artifactView {
        attributes { attribute(artifactType, "android-jni") }
    }.files
    val out = layout.buildDirectory.dir("ane-libs")
    inputs.files(classes, jni)
    outputs.dir(out)
    doLast {
        val dir = out.get().asFile
        dir.deleteRecursively()
        val libs = File(dir, "jni/arm64-v8a").apply { mkdirs() }
        classes.files.sortedBy { it.path }.forEachIndexed { i, jar ->
            jar.copyTo(File(dir, "%02d-%s".format(i, jar.name)))
        }
        jni.files.forEach { folder ->
            File(folder, "arm64-v8a").listFiles { file -> file.name.endsWith(".so") }
                ?.forEach { it.copyTo(File(libs, it.name), overwrite = true) }
        }
    }
}
