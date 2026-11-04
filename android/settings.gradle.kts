pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // rustls-platform-verifier ships its Kotlin half from here, not Maven Central
        maven("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
    }
}

rootProject.name = "Rivet"
include(":app")
