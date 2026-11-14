// The ANE's Android side: a library built on its own (air/ane/build.sh), with Turu's versions.
pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
    versionCatalogs {
        create("libs") {
            from(files("../../../android/gradle/libs.versions.toml"))
        }
    }
}

rootProject.name = "turu-air-ane"

// The Kotlin part of rustls-platform-verifier (the engine's HTTPS checks), from the Ruffle build.
include(":rustls-platform-verifier")
project(":rustls-platform-verifier").projectDir = file("../../../android/rustls-platform-verifier")

// The Kotlin Turu's Android app shares with this (android/shared).
include(":shared")
project(":shared").projectDir = file("../../../android/shared")
