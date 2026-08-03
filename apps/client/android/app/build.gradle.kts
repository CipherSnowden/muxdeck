plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "in.redoimagined.muxdeck_client"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "in.redoimagined.muxdeck_client"
        // 23, not flutter.minSdkVersion. flutter_secure_storage 10.x and mobile_scanner 7.x
        // both declare a floor of 23; below it the manifest merge fails outright.
        // docs/CLIENT.md §4.
        minSdk = 23
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // Deliberately the debug key, not a placeholder. There is no release keystore
            // because putting one in a public repository would publish the signing key, and
            // releases ship a sideloadable APK rather than a Play Store build.
            //
            // The consequence is worth knowing before it bites: Android identifies an app by
            // its signature, so introducing a real keystore later makes updates fail with a
            // signature mismatch for everyone who installed an earlier build. They would have
            // to uninstall first, losing their paired hosts.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
