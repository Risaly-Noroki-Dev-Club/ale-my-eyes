#!/bin/bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    printf "%b%s%b\n" "$GREEN" "$1" "$NC"
}

log_warn() {
    printf "%b%s%b\n" "$YELLOW" "$1" "$NC"
}

log_error() {
    printf "%b%s%b\n" "$RED" "$1" "$NC" >&2
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log_error "未找到命令: $1"
        exit 1
    fi
}

if [ ! -f "Cargo.toml" ]; then
    log_error "请在项目根目录运行此脚本"
    exit 1
fi

require_command rustup
require_command cargo
require_command keytool
require_command find

ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-/usr/local/lib/android/sdk}}"
if [ ! -d "$ANDROID_SDK_ROOT" ]; then
    log_error "Android SDK 目录不存在: $ANDROID_SDK_ROOT"
    log_error "请设置 ANDROID_SDK_ROOT 或 ANDROID_HOME"
    exit 1
fi

export ANDROID_SDK_ROOT
export ANDROID_HOME="$ANDROID_SDK_ROOT"

if [ -n "${ANDROID_NDK_ROOT:-}" ]; then
    NDK_ROOT="$ANDROID_NDK_ROOT"
elif [ -d "$ANDROID_SDK_ROOT/ndk/25.2.9519653" ]; then
    NDK_ROOT="$ANDROID_SDK_ROOT/ndk/25.2.9519653"
else
    log_error "未找到 Android NDK 25.2.9519653"
    log_error "请安装 NDK 并设置 ANDROID_NDK_ROOT，或放在 $ANDROID_SDK_ROOT/ndk/25.2.9519653"
    exit 1
fi

export ANDROID_NDK_ROOT="$NDK_ROOT"

if ! cargo apk --version >/dev/null 2>&1; then
    log_info "安装 cargo-apk..."
    cargo install cargo-apk
fi

log_info "添加 Android Rust targets..."
rustup target add aarch64-linux-android armv7-linux-androideabi

KEYSTORE_PATH="${CARGO_APK_RELEASE_KEYSTORE:-${RUNNER_TEMP:-/tmp}/ale-my-eyes-android-release.keystore}"
KEYSTORE_PASSWORD="${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-android}"

if [ ! -f "$KEYSTORE_PATH" ]; then
    log_info "生成临时 Android 签名 keystore..."
    keytool -genkeypair -v \
        -storetype PKCS12 \
        -keystore "$KEYSTORE_PATH" \
        -storepass "$KEYSTORE_PASSWORD" \
        -alias androiddebugkey \
        -keypass "$KEYSTORE_PASSWORD" \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1
fi

export CARGO_APK_RELEASE_KEYSTORE="$KEYSTORE_PATH"
export CARGO_APK_RELEASE_KEYSTORE_PASSWORD="$KEYSTORE_PASSWORD"

PACKAGE_DIR="ale-my-eyes-android"
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"

build_target() {
    local target="$1"
    local output_name="$2"

    log_info "构建 ${target}..."
    cargo apk build -p ale-gui --target "$target" --lib --release

    local apk_path
    apk_path=$(find target/release/apk -maxdepth 1 -name '*.apk' | head -n 1)
    if [ -z "$apk_path" ] || [ ! -f "$apk_path" ]; then
        log_error "未找到 ${target} 构建产物 APK"
        exit 1
    fi

    cp "$apk_path" "$PACKAGE_DIR/$output_name"
    log_info "已生成: $PACKAGE_DIR/$output_name"
}

build_target aarch64-linux-android ale-my-eyes-arm64.apk
build_target armv7-linux-androideabi ale-my-eyes-armv7.apk

if command -v zip >/dev/null 2>&1; then
    log_info "创建 zip 压缩包..."
    rm -f ale-my-eyes-android.zip
    zip -rq ale-my-eyes-android.zip "$PACKAGE_DIR"
elif command -v tar >/dev/null 2>&1; then
    log_info "创建 tar.gz 压缩包..."
    rm -f ale-my-eyes-android.tar.gz
    tar -czf ale-my-eyes-android.tar.gz "$PACKAGE_DIR"
else
    log_warn "未找到 zip 或 tar，跳过压缩包创建"
fi

log_info "Android 打包完成"
printf "输出目录: %s\n" "$PACKAGE_DIR"
printf "arm64 APK: %s/%s\n" "$PACKAGE_DIR" "ale-my-eyes-arm64.apk"
printf "armv7 APK: %s/%s\n" "$PACKAGE_DIR" "ale-my-eyes-armv7.apk"
