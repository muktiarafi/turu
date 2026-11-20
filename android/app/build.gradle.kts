import java.util.Properties
import javax.inject.Inject

plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.composeCompiler)
    alias(libs.plugins.kotlinSerialization)
}

android {
    namespace = "com.muktiarafi.turu"
    compileSdk = 36
    ndkVersion = "27.3.13750724"

    defaultConfig {
        applicationId = "com.muktiarafi.turu"
        minSdk = 26
        targetSdk = 36
        versionName = turuVersion()
        versionCode = versionCode(turuVersion())
        ndk {
            abiFilters.addAll(ndkAbis())
        }
        // Crash reports to Sentry (`CrashUploads.kt`), sent only once the user said yes.
        // Without a DSN (a build from source) Turu has no uploads at all and never asks.
        buildConfigField("String", "SENTRY_DSN", "\"${sentryDsn()}\"")
    }

    buildFeatures {
        buildConfig = true
        compose = true
    }

    // Release signing: the keystore CI decodes from its secrets (`TURU_KEYSTORE` and the rest,
    // see docs/ANDROID.md). Without it a release build is signed with the debug key.
    signingConfigs {
        System.getenv("TURU_KEYSTORE")?.let { keystore ->
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("TURU_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("TURU_KEY_ALIAS")
                keyPassword = System.getenv("TURU_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
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

// The app's code is Kotlin (`src/main/kotlin`), compiled by AGP 9's built-in Kotlin.
kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
    }
}

dependencies {
    implementation(project(":rustls-platform-verifier"))
    // What Turu Mas shares: crash records and uploads, the crash question, the native text field.
    implementation(project(":shared"))
    // TuruActivity's base (android-activity's `game-activity` feature).
    implementation(libs.gamesActivity)
    implementation(libs.appcompat)
    implementation(libs.core)
    // Crash reports: Java and native crashes (its NDK part), ANRs and tombstones.
    implementation(libs.sentryAndroid)
    // The shell's own screens: the Tabs item and its panel (docs/UI.md, "Native shells").
    implementation(platform(libs.composeBom))
    implementation(libs.composeUi)
    implementation(libs.composeFoundation)
    implementation(libs.composeMaterial3)
    implementation(libs.composeIconsCore)
    implementation(libs.activityCompose)
    implementation(libs.kotlinxSerializationJson)
    implementation(libs.composeUiToolingPreview)
    debugImplementation(libs.composeUiTooling)
}

/// The version: `-PturuVersion=0.2.0-alpha.1` (the release tag without its `v`), else the
/// workspace's in the root `Cargo.toml`.
fun turuVersion(): String =
    findProperty("turuVersion") as String?
        ?: Regex("^version = \"(.+)\"$", RegexOption.MULTILINE)
            .find(rootProject.file("../Cargo.toml").readText())!!.groupValues[1]

/// Android's version code from the version, bigger for every later one: major, minor and patch,
/// then alpha 1, beta 2, rc 3, nightly 8 or a release 9, then the pre-release's number:
/// 0.2.0-alpha.1 is 200101, 0.2.0-beta.3 200203, 0.2.0 200900, 1.0.0 10000900. A nightly
/// (release.yml: 0.4.0-nightly.20261118) is 400800 whatever its date: above 0.4.0's pre-releases,
/// so it installs over them, and below 0.4.0, which installs over it.
fun versionCode(version: String): Int {
    val match = Regex("^(\\d+)\\.(\\d+)\\.(\\d+)(?:-(alpha|beta|rc|nightly)\\.?(\\d*))?").find(version) ?: return 1
    val (major, minor, patch, stage, number) = match.destructured
    val stageCode = when (stage) {
        "alpha" -> 1
        "beta" -> 2
        "rc" -> 3
        "nightly" -> 8
        else -> 9
    }
    val pre = if (stage.isEmpty() || stage == "nightly") 0 else number.ifEmpty { "0" }.toInt()
    return ((major.toInt() * 100 + minor.toInt()) * 100 + patch.toInt()) * 1000 + stageCode * 100 + pre
}

/// Sentry's DSN for crash reports: `-PsentryDsn=...`, the `SENTRY_DSN` environment variable (CI
/// passes its secret), or `sentryDsn` in local.properties; empty for none.
fun sentryDsn(): String {
    val file = rootProject.file("local.properties")
    val local = Properties()
    if (file.exists()) file.inputStream().use { local.load(it) }
    return (findProperty("sentryDsn") as String?
        ?: System.getenv("SENTRY_DSN")
        ?: local.getProperty("sentryDsn")
        ?: "").trim()
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

/**
 * The view models' Kotlin types (docs/NATIVE_UI_PROPOSAL.md, L4), generated from the Rust records
 * (`turu_frontend::models`) by `turu-typegen` on the build machine, into the build folder: never
 * committed, so a record changed in Rust can't ship with stale types here.
 */
abstract class TuruTypes : DefaultTask() {
    @get:InputFiles
    abstract val sources: ConfigurableFileCollection

    @get:Internal
    abstract val workspace: DirectoryProperty

    @get:OutputDirectory
    abstract val outDir: DirectoryProperty

    @get:Inject
    abstract val exec: ExecOperations

    @TaskAction
    fun generate() {
        val out = outDir.get().asFile
        out.deleteRecursively()
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(
                "cargo", "run", "-q", "-p", "turu_frontend", "--no-default-features", "--features", "typegen",
                "--bin", "turu-typegen", "--", out.absolutePath,
            )
        }
        // The generator writes a Gradle project of its own: its sources are what's wanted.
        out.resolve("build.gradle.kts").delete()
    }
}

val turuTypes = tasks.register<TuruTypes>("turuTypes") {
    workspace.set(rootProject.layout.projectDirectory.dir(".."))
    sources.from(rootProject.fileTree("../frontend/src/models"), rootProject.file("../frontend/src/bin/turu-typegen.rs"))
    outDir.set(layout.buildDirectory.dir("generated/turu-models"))
}

androidComponents {
    onVariants { variant ->
        variant.sources.kotlin?.addGeneratedSourceDirectory(turuTypes, TuruTypes::outDir)
    }
}
