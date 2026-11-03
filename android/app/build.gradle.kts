import java.util.Properties

plugins {
    alias(libs.plugins.androidApplication)
}

android {
    namespace = "com.muktiarafi.turu"
    compileSdk = 36
    ndkVersion = "27.3.13750724"

    defaultConfig {
        applicationId = "com.muktiarafi.turu"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters.addAll(ndkAbis())
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    packaging {
        jniLibs {
            // cargo-ndk copies every cdylib the build made; oxc_sourcemap is one, unused.
            excludes += "**/liboxc_sourcemap-*.so"
        }
    }
}

dependencies {
    implementation(project(":rustls-platform-verifier"))
}

/// The ABIs to build: `ndkAbis` in local.properties (space-separated), else the phone's and the
/// emulator's. 32-bit ABIs are left out (see docs/ANDROID_PROPOSAL.md).
fun ndkAbis(): List<String> {
    val file = rootProject.file("local.properties")
    val local = Properties()
    if (file.exists()) file.inputStream().use { local.load(it) }
    return (local.getProperty("ndkAbis") ?: "arm64-v8a x86_64").split(Regex("\\s+")).filter { it.isNotEmpty() }
}

// The Rust library, built with cargo-ndk into jniLibs/<abi>/libturu_android.so. Release
// profile: a debug build of the engine and QuickJS is too slow to play.
val cargoNdk = tasks.register<Exec>("cargoNdk") {
    workingDir = rootProject.projectDir
    val args = mutableListOf("cargo", "ndk", "--platform", "26", "-o", "app/src/main/jniLibs")
    for (abi in ndkAbis()) {
        args += listOf("-t", abi)
    }
    args += listOf("build", "-p", "turu_android", "--release")
    commandLine(args)
    val ndkDirectory = androidComponents.sdkComponents.ndkDirectory
    doFirst {
        val ndk = ndkDirectory.get().asFile
        val os = System.getProperty("os.name").lowercase()
        val host = when {
            os.contains("mac") -> "darwin-x86_64"
            os.contains("windows") -> "windows-x86_64"
            else -> "linux-x86_64"
        }
        environment("ANDROID_NDK_HOME", ndk.absolutePath)
        // rquickjs generates its bindings with bindgen: libclang (macOS: the Command Line Tools'
        // unless LIBCLANG_PATH says otherwise) and the NDK's headers.
        environment("BINDGEN_EXTRA_CLANG_ARGS", "--sysroot=${ndk.resolve("toolchains/llvm/prebuilt/$host/sysroot")}")
        if (System.getenv("LIBCLANG_PATH") == null && host == "darwin-x86_64") {
            environment("LIBCLANG_PATH", "/Library/Developer/CommandLineTools/usr/lib")
        }
    }
}

tasks.named("preBuild") {
    dependsOn(cargoNdk)
}
