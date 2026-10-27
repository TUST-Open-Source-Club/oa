plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.cluboahq.app"
    compileSdk = 35
    defaultConfig {
        applicationId = "com.cluboahq.app"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.webkit:webkit:1.12.1")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // 推送 SDK（待厂商资质后逐步启用，按 vendor 装载）：
    // implementation("com.huawei.hms:push:6.13.0.300")
    // implementation("com.xiaomi.mipush.sdk:MiPush:5.9.9")
    // implementation("com.heytap.msp:push:3.5.2")
    // implementation("com.vivo.push:PushSDK:3.0.0.7")
    // implementation("com.meizu.flyme.internet:push-internal:4.1.4")
    // implementation("com.hihonor.mcs:push:7.0.61.302")
    // implementation(platform("com.google.firebase:firebase-bom:33.7.0"))
    // implementation("com.google.firebase:firebase-messaging")
}
