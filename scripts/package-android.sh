#!/bin/bash
# Ale, My Eyes! Android 打包脚本
# 用法: ./scripts/package-android.sh

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}开始打包 Ale, My Eyes! Android 版本...${NC}"

# 检查是否在项目根目录
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}错误: 请在项目根目录运行此脚本${NC}"
    exit 1
fi

# 检查 Rust 工具链
if ! command -v rustup &> /dev/null; then
    echo -e "${RED}错误: 未找到 rustup，请先安装 Rust${NC}"
    exit 1
fi

# 检查 cargo-ndk
if ! command -v cargo-ndk &> /dev/null; then
    echo -e "${YELLOW}安装 cargo-ndk...${NC}"
    cargo install cargo-ndk
fi

# 检查 Android NDK
if [ -z "$ANDROID_NDK_ROOT" ]; then
    echo -e "${RED}错误: 未设置 ANDROID_NDK_ROOT 环境变量${NC}"
    echo -e "${YELLOW}请安装 Android NDK 并设置环境变量：${NC}"
    echo -e "export ANDROID_NDK_ROOT=/path/to/android-ndk"
    exit 1
fi

# 添加 Android 目标
echo -e "${YELLOW}添加 Android 目标...${NC}"
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add x86_64-linux-android
rustup target add i686-linux-android

# 构建 Android 版本
echo -e "${YELLOW}构建 Android 版本...${NC}"

# 构建 arm64 版本
echo -e "${YELLOW}构建 arm64 版本...${NC}"
cargo ndk -t arm64-v8a build -p ale-core --release

# 构建 armv7 版本
echo -e "${YELLOW}构建 armv7 版本...${NC}"
cargo ndk -t armeabi-v7a build -p ale-core --release

# 创建打包目录
PACKAGE_DIR="ale-my-eyes-android"
echo -e "${YELLOW}创建打包目录: ${PACKAGE_DIR}${NC}"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}"

# 创建 APK 结构
echo -e "${YELLOW}创建 APK 结构...${NC}"
mkdir -p "${PACKAGE_DIR}/app/src/main/java/com/alemyeyes"
mkdir -p "${PACKAGE_DIR}/app/src/main/assets"
mkdir -p "${PACKAGE_DIR}/app/src/main/jniLibs/arm64-v8a"
mkdir -p "${PACKAGE_DIR}/app/src/main/jniLibs/armeabi-v7a"
mkdir -p "${PACKAGE_DIR}/app/src/main/res/values"
mkdir -p "${PACKAGE_DIR}/app/src/main/res/layout"

# 复制 native 库
echo -e "${YELLOW}复制 native 库...${NC}"
cp target/aarch64-linux-android/release/libale_core.so "${PACKAGE_DIR}/app/src/main/jniLibs/arm64-v8a/"
cp target/armv7-linux-androideabi/release/libale_core.so "${PACKAGE_DIR}/app/src/main/jniLibs/armeabi-v7a/"

# 创建 AndroidManifest.xml
echo -e "${YELLOW}创建 AndroidManifest.xml...${NC}"
cat > "${PACKAGE_DIR}/app/src/main/AndroidManifest.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.alemyeyes">

    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.RECORD_AUDIO" />
    <uses-permission android:name="android.permission.MODIFY_AUDIO_SETTINGS" />
    <uses-permission android:name="android.permission.CAMERA" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" />
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" />

    <application
        android:allowBackup="true"
        android:label="@string/app_name"
        android:supportsRtl="true"
        android:theme="@style/AppTheme">
        
        <activity
            android:name=".MainActivity"
            android:exported="true"
            android:screenOrientation="portrait">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
        
        <service
            android:name=".AleService"
            android:exported="false" />
            
    </application>
</manifest>
EOF

# 创建最小可运行 Activity 和 Service
echo -e "${YELLOW}创建 Android Activity...${NC}"
cat > "${PACKAGE_DIR}/app/src/main/java/com/alemyeyes/MainActivity.kt" << EOF
package com.alemyeyes

import android.app.Activity
import android.os.Bundle
import android.view.Gravity
import android.widget.TextView

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val textView = TextView(this).apply {
            text = "Ale, My Eyes!"
            textSize = 24f
            gravity = Gravity.CENTER
        }

        setContentView(textView)
    }
}
EOF

cat > "${PACKAGE_DIR}/app/src/main/java/com/alemyeyes/AleService.kt" << EOF
package com.alemyeyes

import android.app.Service
import android.content.Intent
import android.os.IBinder

class AleService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null
}
EOF

# 创建 strings.xml
echo -e "${YELLOW}创建 strings.xml...${NC}"
cat > "${PACKAGE_DIR}/app/src/main/res/values/strings.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="app_name">Ale, My Eyes!</string>
    <string name="start_recording">开始录音</string>
    <string name="stop_recording">停止录音</string>
    <string name="describe_image">描述图像</string>
    <string name="settings">设置</string>
    <string name="api_key">API 密钥</string>
    <string name="save">保存</string>
    <string name="cancel">取消</string>
</resources>
EOF

# 创建主题资源
echo -e "${YELLOW}创建 styles.xml...${NC}"
cat > "${PACKAGE_DIR}/app/src/main/res/values/styles.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="AppTheme" parent="android:style/Theme.Material.Light.NoActionBar" />
</resources>
EOF

# 创建 build.gradle
echo -e "${YELLOW}创建 build.gradle...${NC}"
cat > "${PACKAGE_DIR}/app/build.gradle" << EOF
plugins {
    id 'com.android.application'
    id 'org.jetbrains.kotlin.android'
}

android {
    namespace 'com.alemyeyes'
    compileSdk 34

    defaultConfig {
        applicationId "com.alemyeyes"
        minSdk 26
        targetSdk 34
        versionCode 1
        versionName "1.0"

        ndk {
            abiFilters 'arm64-v8a', 'armeabi-v7a'
        }
    }

    buildTypes {
        release {
            minifyEnabled false
            proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
        }
    }
    
    compileOptions {
        sourceCompatibility JavaVersion.VERSION_1_8
        targetCompatibility JavaVersion.VERSION_1_8
    }
    
    kotlinOptions {
        jvmTarget = '1.8'
    }
}

dependencies {
    implementation 'androidx.core:core-ktx:1.12.0'
    implementation 'androidx.appcompat:appcompat:1.6.1'
    implementation 'com.google.android.material:material:1.11.0'
    implementation 'androidx.constraintlayout:constraintlayout:2.1.4'
}
EOF

# 创建项目级 build.gradle
echo -e "${YELLOW}创建项目级 build.gradle...${NC}"
cat > "${PACKAGE_DIR}/build.gradle" << EOF
buildscript {
    ext.kotlin_version = '1.9.22'
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath 'com.android.tools.build:gradle:8.2.2'
        classpath "org.jetbrains.kotlin:kotlin-gradle-plugin:\$kotlin_version"
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

task clean(type: Delete) {
    delete rootProject.buildDir
}
EOF

# 创建 settings.gradle
echo -e "${YELLOW}创建 settings.gradle...${NC}"
cat > "${PACKAGE_DIR}/settings.gradle" << EOF
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
    }
}

rootProject.name = "AleMyEyes"
include ':app'
EOF

# 创建 gradle.properties
echo -e "${YELLOW}创建 gradle.properties...${NC}"
cat > "${PACKAGE_DIR}/gradle.properties" << EOF
org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
kotlin.code.style=official
android.nonTransitiveRClass=true
EOF

# 创建 gradle wrapper
echo -e "${YELLOW}创建 gradle wrapper...${NC}"
mkdir -p "${PACKAGE_DIR}/gradle/wrapper"
cat > "${PACKAGE_DIR}/gradle/wrapper/gradle-wrapper.properties" << EOF
distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://services.gradle.org/distributions/gradle-8.5-bin.zip
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
EOF

# 创建 proguard-rules.pro
echo -e "${YELLOW}创建 proguard-rules.pro...${NC}"
cat > "${PACKAGE_DIR}/app/proguard-rules.pro" << EOF
# Add project specific ProGuard rules here.
-keep class com.alemyeyes.** { *; }
EOF

# 创建 README
echo -e "${YELLOW}创建 README...${NC}"
cat > "${PACKAGE_DIR}/README.md" << EOF
# Ale, My Eyes! - Android 版本

这是一个为视障人士设计的智能辅助系统的 Android 版本。

## 构建要求

- Android Studio 2023.1.1 或更高版本
- Android SDK 34
- Android NDK 25.2.9519653 或更高版本
- Rust 工具链

## 构建步骤

1. 克隆项目
2. 设置环境变量：
   \`\`\`bash
   export ANDROID_NDK_ROOT=/path/to/android-ndk
   \`\`\`
3. 运行打包脚本：
   \`\`\`bash
   ./scripts/package-android.sh
   \`\`\`
4. 在 Android Studio 中打开项目
5. 构建并运行

## 功能特性

- 语音识别：通过麦克风输入语音指令
- 语音合成：系统状态和屏幕内容的语音反馈
- 图像描述：使用相机拍摄图像并获取描述
- 自然语言交互：支持自然语言指令

## 系统要求

- Android 8.0 (API 26) 或更高版本
- 至少 4GB 内存
- 麦克风和扬声器
- 相机（可选，用于图像描述）
- 网络连接（用于云端 API）

## 配置说明

首次启动时，应用会提示您输入 API 密钥。您也可以在设置中修改配置。

## 获取帮助

如需帮助，请访问项目主页或提交 Issue。
EOF

# 创建压缩包
echo -e "${YELLOW}创建压缩包...${NC}"
if command -v zip &> /dev/null; then
    zip -r "ale-my-eyes-android.zip" "${PACKAGE_DIR}"
elif command -v tar &> /dev/null; then
    tar -czf "ale-my-eyes-android.tar.gz" "${PACKAGE_DIR}"
else
    echo -e "${YELLOW}警告: 未找到 zip 或 tar，跳过压缩包创建${NC}"
fi

echo -e "${GREEN}打包完成！${NC}"
echo -e "${GREEN}输出目录: ${PACKAGE_DIR}${NC}"
if [ -f "ale-my-eyes-android.zip" ]; then
    echo -e "${GREEN}压缩包: ale-my-eyes-android.zip${NC}"
fi

echo -e "${YELLOW}下一步：${NC}"
echo -e "1. 将 ${PACKAGE_DIR} 目录复制到开发机器"
echo -e "2. 在 Android Studio 中打开项目"
echo -e "3. 设置 Android NDK 路径"
echo -e "4. 构建并运行应用"
