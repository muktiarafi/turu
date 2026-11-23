import java.util.Properties
import java.util.UUID
import javax.inject.Inject

plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.composeCompiler)
    alias(libs.plugins.kotlinSerialization)
}

android {
    namespace = "com.muktiarafi.turu"
    // Compose 1.13 (BOM 2026.11.01, Material 3 1.5.0) needs compileSdk 37.1. Only this module:
    // `shared` and Turu Mas's extension stay on 36. targetSdk stays 36 (no behaviour change).
    compileSdk {
        version = release(37) { minorApiLevel = 1 }
    }
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
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    packaging {
        jniLibs {
            // cargo-ndk copies every cdylib the build made; oxc_sourcemap is one, unused (hashed names
            // locally, `liboxc_sourcemap.so` on CI).
            excludes += "**/liboxc_sourcemap*.so"
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
    testImplementation(libs.junit)
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
 * committed, so a record changed in Rust can't ship with stale types here. With them, each model's
 * view as Rust writes it (`samplesDir`, from a fresh tab with no game), which the JVM tests decode
 * into the generated types (`GeneratedTypesTest`).
 */
abstract class TuruTypes : DefaultTask() {
    @get:InputFiles
    abstract val sources: ConfigurableFileCollection

    @get:Internal
    abstract val workspace: DirectoryProperty

    @get:OutputDirectory
    abstract val outDir: DirectoryProperty

    @get:OutputDirectory
    abstract val samplesDir: DirectoryProperty

    @get:Inject
    abstract val exec: ExecOperations

    @TaskAction
    fun generate() {
        val out = outDir.get().asFile
        val samples = samplesDir.get().asFile
        out.deleteRecursively()
        samples.deleteRecursively()
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(
                "cargo", "run", "-q", "-p", "turu_frontend", "--no-default-features", "--features", "typegen",
                "--bin", "turu-typegen", "--", out.absolutePath, "--samples", samples.absolutePath,
            )
        }
        // The generator writes a Gradle project of its own: its sources are what's wanted.
        out.resolve("build.gradle.kts").delete()
    }
}

val turuTypes = tasks.register<TuruTypes>("turuTypes") {
    workspace.set(rootProject.layout.projectDirectory.dir(".."))
    // Every Rust file the types come from: the models, the shell's typed JNI (`native_shell.rs`)
    // and the link's types it uses (`shell.rs`), and the generator.
    sources.from(
        rootProject.fileTree("../frontend/src/models"),
        rootProject.file("../frontend/src/native_shell.rs"),
        rootProject.file("../frontend/src/shell.rs"),
        rootProject.file("../frontend/src/bin/turu-typegen.rs"),
    )
    outDir.set(layout.buildDirectory.dir("generated/turu-models"))
    samplesDir.set(layout.buildDirectory.dir("generated/turu-samples"))
}

// The JVM tests read the views Rust wrote beside the types.
tasks.withType<Test>().configureEach {
    dependsOn(turuTypes)
    val samples = turuTypes.flatMap { it.samplesDir }
    inputs.dir(samples)
    systemProperty("turu.samples", samples.get().asFile.absolutePath)
}

/**
 * The network security config: rustls-platform-verifier's (`res/xml/network_security_config.xml`:
 * plain HTTP to the certificate authorities' revocation lists, which the certificate checks fetch)
 * with loopback added, where the shell reads its slots' control APIs. An app resource of the same
 * name replaces the library's, so it's made from the library's at build time, never copied by
 * hand: one written by hand dropped the revocation lists and every HTTPS check failed.
 */
abstract class TuruNetworkConfig : DefaultTask() {
    @get:InputFile
    abstract val library: RegularFileProperty

    @get:OutputDirectory
    abstract val outDir: DirectoryProperty

    @TaskAction
    fun generate() {
        val text = library.get().asFile.readText()
        val loopback = """
  <!-- Turu: the shell reads its slots' control APIs over loopback (each with its own token). -->
  <domain-config cleartextTrafficPermitted="true">
    <domain includeSubdomains="false">127.0.0.1</domain>
  </domain-config>
</network-security-config>"""
        check(text.contains("</network-security-config>")) { "rustls-platform-verifier's config changed shape" }
        val out = outDir.get().asFile.resolve("xml/network_security_config.xml")
        out.parentFile.mkdirs()
        out.writeText(text.replace("</network-security-config>", loopback.trimStart('\n')))
    }
}

val turuNetworkConfig = tasks.register<TuruNetworkConfig>("turuNetworkConfig") {
    library.set(rootProject.layout.projectDirectory.file(
        "vendor/rustls-platform-verifier/android/rustls-platform-verifier/src/main/res/xml/network_security_config.xml",
    ))
    outDir.set(layout.buildDirectory.dir("generated/turu-res"))
}

/**
 * The R8 mapping's ID for Sentry: a new UUID each release build, in the APK's
 * `assets/sentry-debug-meta.properties` (sentry-android reads it and sends it with each crash
 * report) and beside the mapping (`build/outputs/mapping/release/sentry-uuid.txt`), from where
 * release.yml uploads `mapping.txt` under it. With the mapping Sentry puts back the frames R8
 * inlined. The mapping and the UUID belong to one build: upload the mapping of the build whose
 * APK ships.
 */
abstract class SentryProguardUuid : DefaultTask() {
    @get:OutputDirectory
    abstract val outDir: DirectoryProperty

    @get:OutputFile
    abstract val uuidFile: RegularFileProperty

    @TaskAction
    fun generate() {
        val uuid = UUID.randomUUID().toString()
        val out = outDir.get().asFile
        out.mkdirs()
        out.resolve("sentry-debug-meta.properties").writeText("io.sentry.ProguardUuids=$uuid\n")
        uuidFile.get().asFile.writeText("$uuid\n")
    }
}

val sentryProguardUuid = tasks.register<SentryProguardUuid>("sentryProguardUuid") {
    outDir.set(layout.buildDirectory.dir("generated/sentry-assets"))
    uuidFile.set(layout.buildDirectory.file("outputs/mapping/release/sentry-uuid.txt"))
    // A new one each build: two builds' mappings never share an ID.
    outputs.upToDateWhen { false }
}

androidComponents {
    onVariants { variant ->
        variant.sources.kotlin?.addGeneratedSourceDirectory(turuTypes, TuruTypes::outDir)
        variant.sources.res?.addGeneratedSourceDirectory(turuNetworkConfig, TuruNetworkConfig::outDir)
    }
    onVariants(selector().withBuildType("release")) { variant ->
        variant.sources.assets?.addGeneratedSourceDirectory(sentryProguardUuid, SentryProguardUuid::outDir)
    }
}
