plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Resolve the rustls-platform-verifier Android component version from the
// Cargo lockfile so the Kotlin and Rust halves can never drift apart.
abstract class RustlsVersion : ValueSource<String, RustlsVersion.Params> {
    interface Params : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    override fun obtain(): String {
        val crate = "rustls-platform-verifier-android"
        val lines = parameters.lockFile.get().asFile.readLines()
        val idx = lines.indexOfFirst { it.trim() == "name = \"$crate\"" }
        val version = if (idx < 0) null else lines.drop(idx + 1)
            .firstOrNull { it.trimStart().startsWith("version = ") }
            ?.substringAfter('"', "")
            ?.substringBefore('"', "")
            ?.takeIf { it.isNotEmpty() }
        return version ?: throw GradleException("$crate not found in Cargo.lock")
    }
}

val rustlsPlatformVerifierVersion = providers.of(RustlsVersion::class) {
    parameters.lockFile.set(layout.projectDirectory.file("../../Cargo.lock"))
}

configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "rustls" && requested.name == "rustls-platform-verifier") {
            useVersion(rustlsPlatformVerifierVersion.get())
            because("Kotlin component version must match the Rust crate")
        }
    }
}

android {
    namespace = "dev.hori.torrent"
    compileSdk = 35
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "dev.hori.torrent"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            // ABIs we ship native libraries for (built by cargo-ndk in CI).
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    // Native libraries land here via `cargo ndk -o android/app/src/main/jniLibs`.
    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")

    signingConfigs {
        // CI passes a throwaway keystore through env vars; locally we fall back
        // to the debug keystore so `assembleDebug` works out of the box.
        create("ci") {
            val ksPath = System.getenv("HORI_KEYSTORE")
            if (ksPath != null && file(ksPath).exists()) {
                storeFile = file(ksPath)
                storePassword = System.getenv("HORI_KEYSTORE_PASSWORD") ?: ""
                keyAlias = System.getenv("HORI_KEY_ALIAS") ?: "hori"
                keyPassword = System.getenv("HORI_KEY_PASSWORD") ?: ""
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            val ksPath = System.getenv("HORI_KEYSTORE")
            signingConfig = if (ksPath != null && file(ksPath).exists()) {
                signingConfigs.getByName("ci")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // Version resolved dynamically from Cargo.lock above.
    implementation("rustls:rustls-platform-verifier")
}
