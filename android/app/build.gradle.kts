import javax.inject.Inject

plugins {
    id("com.android.application")
}

// ok so the Kotlin half of rustls-platform-verifier has to match the crate
// version cargo picked, which only Cargo.lock knows
val workspaceDir = rootDir.parentFile
val verifierVersion: String = workspaceDir.resolve("Cargo.lock").readLines().let { lines ->
    val at = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
    require(at >= 0) { "rustls-platform-verifier-android not found in Cargo.lock" }
    lines.drop(at + 1).first { it.trimStart().startsWith("version = ") }.substringAfter('"').substringBefore('"')
}

// the app version lives in the root Cargo.toml, the apk just follows it
val appVersion: String = workspaceDir.resolve("Cargo.toml").readLines()
    .first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
val appVersionCode = appVersion.substringBefore('-').split('.').map(String::toInt).let { (major, minor, patch) ->
    major * 10_000 + minor * 100 + patch
}

// release signing only kicks in when CI (or you) hand over a keystore
val keystore = providers.environmentVariable("RIVET_KEYSTORE").orNull

android {
    namespace = "dev.rivet"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "dev.rivet"
        minSdk = 26
        targetSdk = 35
        versionCode = appVersionCode
        versionName = appVersion
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = providers.environmentVariable("RIVET_KEYSTORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("RIVET_KEY_ALIAS").get()
                keyPassword = providers.environmentVariable("RIVET_KEY_PASSWORD").get()
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (keystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.8.0")
    // android-activity 0.6 speaks the 4.x GameActivity glue
    implementation("androidx.games:games-activity:4.4.0")
    implementation("org.rustls:rustls-platform-verifier:$verifierVersion")
}

/** Builds the Rust library for each ABI and drops it, plus libc++_shared, into jniLibs. */
abstract class CargoBuild @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val minSdk: Property<Int>
    @get:Internal abstract val ndkDir: DirectoryProperty
    @get:Internal abstract val workspace: DirectoryProperty
    @get:Internal abstract val jniLibs: DirectoryProperty

    @TaskAction
    fun build() {
        val triples = mapOf(
            "arm64-v8a" to "aarch64-linux-android",
            "x86_64" to "x86_64-linux-android",
        )
        val ndk = ndkDir.get().asFile
        val bin = ndk.resolve("toolchains/llvm/prebuilt/linux-x86_64/bin")
        val sysroot = ndk.resolve("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib")
        for (abi in abis.get()) {
            val triple = triples[abi] ?: throw GradleException("unsupported abi $abi")
            val clang = bin.resolve("$triple${minSdk.get()}-clang").absolutePath
            val key = triple.replace('-', '_')
            exec.exec {
                workingDir = workspace.get().asFile
                commandLine("cargo", "build", "--release", "-p", "rivet-android", "--target", triple)
                environment("ANDROID_NDK_HOME", ndk.absolutePath)
                environment("CC_$key", clang)
                environment("CXX_$key", "$clang++")
                environment("AR_$key", bin.resolve("llvm-ar").absolutePath)
                environment("RANLIB_$key", bin.resolve("llvm-ranlib").absolutePath)
                environment("CARGO_TARGET_${key.uppercase()}_LINKER", clang)
            }
            val out = jniLibs.get().asFile.resolve(abi).apply { mkdirs() }
            workspace.get().asFile.resolve("target/$triple/release/librivet_android.so").copyTo(out.resolve("librivet_android.so"), true)
            // GameActivity's glue is C++, so the shared STL has to ship next to us
            sysroot.resolve("$triple/libc++_shared.so").copyTo(out.resolve("libc++_shared.so"), true)
        }
    }
}

val cargoBuild = tasks.register<CargoBuild>("cargoBuild") {
    val requested = providers.gradleProperty("rivet.abis").orElse("arm64-v8a,x86_64")
    abis.set(requested.map { it.split(',').map(String::trim).filter(String::isNotEmpty) })
    minSdk.set(26)
    ndkDir.set(androidComponents.sdkComponents.ndkDirectory)
    workspace.set(workspaceDir)
    jniLibs.set(layout.projectDirectory.dir("src/main/jniLibs"))
    // cargo tracks its own inputs, so always hand over to it
    outputs.upToDateWhen { false }
}

tasks.named("preBuild") { dependsOn(cargoBuild) }
