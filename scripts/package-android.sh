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

import android.Manifest
import android.content.pm.PackageManager
import android.media.MediaRecorder
import android.os.Bundle
import android.speech.tts.TextToSpeech
import android.util.Base64
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale

class MainActivity : ComponentActivity(), TextToSpeech.OnInitListener {
    private lateinit var apiKeyInput: EditText
    private lateinit var resultView: TextView
    private var recorder: MediaRecorder? = null
    private var recordingFile: File? = null
    private var tts: TextToSpeech? = null

    private val imagePicker = registerForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) {
            describeImage(uri)
        } else {
            setResultText("未选择图片")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        tts = TextToSpeech(this, this)

        val prefs = getSharedPreferences("ale-my-eyes", MODE_PRIVATE)

        apiKeyInput = EditText(this).apply {
            hint = "OpenAI API Key"
            setText(prefs.getString("api_key", ""))
        }

        resultView = TextView(this).apply {
            text = "就绪"
            textSize = 18f
            setPadding(0, 16, 0, 16)
        }

        val saveButton = Button(this).apply {
            text = "保存 API Key"
            setOnClickListener {
                prefs.edit().putString("api_key", apiKeyInput.text.toString()).apply()
                setResultText("API Key 已保存")
            }
        }

        val describeButton = Button(this).apply {
            text = "选择图片并描述"
            setOnClickListener { imagePicker.launch("image/*") }
        }

        val recordButton = Button(this).apply {
            text = "开始录音"
            setOnClickListener {
                if (recorder == null) {
                    startRecording(this)
                } else {
                    stopRecording(this)
                }
            }
        }

        val speakButton = Button(this).apply {
            text = "朗读结果"
            setOnClickListener { speak(resultView.text.toString()) }
        }

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 32, 32, 32)
            addView(TextView(this@MainActivity).apply {
                text = "Ale, My Eyes!"
                textSize = 26f
            })
            addView(apiKeyInput)
            addView(saveButton)
            addView(describeButton)
            addView(recordButton)
            addView(speakButton)
            addView(resultView)
        }

        setContentView(ScrollView(this).apply { addView(layout) })
    }

    override fun onInit(status: Int) {
        if (status == TextToSpeech.SUCCESS) {
            tts?.language = Locale.CHINESE
        }
    }

    private fun apiKey(): String = apiKeyInput.text.toString().trim()

    private fun setResultText(text: String) {
        runOnUiThread { resultView.text = text }
    }

    private fun describeImage(uri: android.net.Uri) {
        val key = apiKey()
        if (key.isEmpty()) {
            setResultText("请先保存 API Key")
            return
        }

        setResultText("正在描述图片...")
        Thread {
            try {
                val imageBytes = contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: throw IllegalStateException("无法读取图片")
                val imageBase64 = Base64.encodeToString(imageBytes, Base64.NO_WRAP)
                val body = JSONObject()
                    .put("model", "gpt-4o")
                    .put("max_tokens", 1024)
                    .put("messages", JSONArray().put(JSONObject()
                        .put("role", "user")
                        .put("content", JSONArray()
                            .put(JSONObject().put("type", "text").put("text", "请描述这张图片的内容"))
                            .put(JSONObject().put("type", "image_url").put("image_url", JSONObject().put("url", "data:image/jpeg;base64," + imageBase64))))))
                val response = postJson("https://api.openai.com/v1/chat/completions", key, body)
                val text = JSONObject(response)
                    .getJSONArray("choices")
                    .getJSONObject(0)
                    .getJSONObject("message")
                    .getString("content")
                setResultText(text)
            } catch (error: Exception) {
                setResultText("图片描述失败: " + error.message)
            }
        }.start()
    }

    private fun startRecording(button: Button) {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            ActivityCompat.requestPermissions(this, arrayOf(Manifest.permission.RECORD_AUDIO), 1)
            return
        }

        recordingFile = File(cacheDir, "recording.m4a")
        recorder = MediaRecorder().apply {
            setAudioSource(MediaRecorder.AudioSource.MIC)
            setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            setOutputFile(recordingFile!!.absolutePath)
            prepare()
            start()
        }
        button.text = "停止录音并转写"
        setResultText("正在录音...")
    }

    private fun stopRecording(button: Button) {
        val key = apiKey()
        if (key.isEmpty()) {
            setResultText("请先保存 API Key")
            return
        }

        recorder?.run {
            stop()
            release()
        }
        recorder = null
        button.text = "开始录音"
        setResultText("正在转写录音...")

        val file = recordingFile ?: return setResultText("录音文件不存在")
        Thread {
            try {
                val response = postMultipartAudio("https://api.openai.com/v1/audio/transcriptions", key, file)
                val text = JSONObject(response).getString("text")
                setResultText(text)
            } catch (error: Exception) {
                setResultText("语音识别失败: " + error.message)
            }
        }.start()
    }

    private fun speak(text: String) {
        tts?.speak(text, TextToSpeech.QUEUE_FLUSH, null, "result")
    }

    private fun postJson(url: String, key: String, body: JSONObject): String {
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.requestMethod = "POST"
        connection.setRequestProperty("Authorization", "Bearer " + key)
        connection.setRequestProperty("Content-Type", "application/json")
        connection.doOutput = true
        OutputStreamWriter(connection.outputStream).use { it.write(body.toString()) }
        return readResponse(connection)
    }

    private fun postMultipartAudio(url: String, key: String, file: File): String {
        val boundary = "AleMyEyesBoundary" + System.currentTimeMillis()
        val connection = URL(url).openConnection() as HttpURLConnection
        connection.requestMethod = "POST"
        connection.setRequestProperty("Authorization", "Bearer " + key)
        connection.setRequestProperty("Content-Type", "multipart/form-data; boundary=" + boundary)
        connection.doOutput = true

        connection.outputStream.use { output ->
            output.write("--".toByteArray())
            output.write(boundary.toByteArray())
            output.write("\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nwhisper-1\r\n".toByteArray())
            output.write("--".toByteArray())
            output.write(boundary.toByteArray())
            output.write("\r\nContent-Disposition: form-data; name=\"file\"; filename=\"recording.m4a\"\r\n".toByteArray())
            output.write("Content-Type: audio/mp4\r\n\r\n".toByteArray())
            output.write(file.readBytes())
            output.write("\r\n--".toByteArray())
            output.write(boundary.toByteArray())
            output.write("--\r\n".toByteArray())
        }

        return readResponse(connection)
    }

    private fun readResponse(connection: HttpURLConnection): String {
        val stream = if (connection.responseCode in 200..299) connection.inputStream else connection.errorStream
        val response = stream.bufferedReader().use { it.readText() }
        if (connection.responseCode !in 200..299) {
            throw IllegalStateException(response)
        }
        return response
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
    implementation 'androidx.activity:activity-ktx:1.8.2'
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
