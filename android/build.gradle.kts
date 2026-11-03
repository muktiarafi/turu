// Turu's Android app (docs/ANDROID_PROPOSAL.md). The Rust library is the `turu_android` crate in
// this folder, built by `app`'s `cargoNdk` task.
plugins {
    alias(libs.plugins.androidApplication) apply false
    alias(libs.plugins.androidLibrary) apply false
    // Pins the Kotlin version AGP builds `rustls-platform-verifier` with.
    alias(libs.plugins.jetbrainsKotlinAndroid) apply false
}
