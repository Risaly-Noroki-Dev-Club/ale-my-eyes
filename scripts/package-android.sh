#!/usr/bin/env bash

set -euo pipefail

RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[1;33m"
NC="\033[0m"

DEFAULT_NDK_VERSION="27.3.13750724"
DEFAULT_TARGETS="aarch64-linux-android"
PACKAGE_DIR="ale-my-eyes-android"
TARGET_ANDROID_API="34"

log_info() {
    printf "%b%s%b\n" "$GREEN" "$1" "$NC"
}

log_warn() {
    printf "%b%s%b\n" "$YELLOW" "$1" "$NC" >&2
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

require_repo_root() {
    if [ ! -f "Cargo.toml" ] || [ ! -d "ale-gui" ]; then
        log_error "请在项目根目录运行此脚本"
        exit 1
    fi
}

is_valid_ndk() {
    local ndk_root="$1"
    [ -d "$ndk_root" ] && [ -f "$ndk_root/source.properties" ]
}

resolve_sdk_root() {
    local candidates=()

    if [ -n "${ANDROID_HOME:-}" ]; then
        candidates+=("$ANDROID_HOME")
    fi
    if [ -n "${ANDROID_SDK_ROOT:-}" ]; then
        candidates+=("$ANDROID_SDK_ROOT")
    fi
    candidates+=("$HOME/Library/Android/sdk")
    candidates+=("/usr/local/lib/android/sdk")
    candidates+=("/opt/android-sdk")

    local sdk_root=""
    for sdk_root in "${candidates[@]}"; do
        if [ -d "$sdk_root" ]; then
            printf "%s\n" "$sdk_root"
            return
        fi
    done

    log_error "未找到 Android SDK"
    log_error "请安装 Android Studio，或设置 ANDROID_HOME=/path/to/android-sdk"
    exit 1
}

resolve_android_jar() {
    local sdk_root="$1"
    local preferred="$sdk_root/platforms/android-$TARGET_ANDROID_API/android.jar"

    if [ -f "$preferred" ]; then
        printf "%s\n" "$preferred"
        return
    fi

    local jar=""
    jar=$(find "$sdk_root/platforms" -mindepth 2 -maxdepth 2 -name android.jar -type f 2>/dev/null | sort -V | tail -n 1 || true)
    if [ -n "$jar" ]; then
        log_warn "未找到 android-$TARGET_ANDROID_API，回退到: $jar"
        printf "%s\n" "$jar"
        return
    fi

    log_error "未找到 android.jar"
    log_error "请在 Android Studio 的 SDK Platforms 中安装 Android API $TARGET_ANDROID_API"
    exit 1
}

resolve_build_tools() {
    local sdk_root="$1"
    local build_tools_dir="$sdk_root/build-tools"
    local newest=""

    if [ ! -d "$build_tools_dir" ]; then
        log_error "未找到 Android build-tools"
        log_error "请在 Android Studio 的 SDK Tools 中安装 Android SDK Build-Tools"
        exit 1
    fi

    newest=$(find "$build_tools_dir" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)
    if [ -z "$newest" ]; then
        log_error "Android build-tools 目录为空: $build_tools_dir"
        exit 1
    fi

    printf "%s\n" "$newest"
}

resolve_ndk_root() {
    local sdk_root="$1"
    local requested_version="${ANDROID_NDK_VERSION:-}"
    local ndk_root=""

    if [ -n "${ANDROID_NDK_ROOT:-}" ]; then
        if is_valid_ndk "$ANDROID_NDK_ROOT"; then
            printf "%s\n" "$ANDROID_NDK_ROOT"
            return
        fi
        log_warn "忽略无效 ANDROID_NDK_ROOT: $ANDROID_NDK_ROOT"
    fi

    if [ -n "$requested_version" ]; then
        ndk_root="$sdk_root/ndk/$requested_version"
        if is_valid_ndk "$ndk_root"; then
            printf "%s\n" "$ndk_root"
            return
        fi
        log_error "未找到可用 Android NDK 版本: $requested_version"
        log_error "请安装该版本，或设置 ANDROID_NDK_ROOT 指向有效 NDK 目录"
        exit 1
    fi

    ndk_root="$sdk_root/ndk/$DEFAULT_NDK_VERSION"
    if is_valid_ndk "$ndk_root"; then
        printf "%s\n" "$ndk_root"
        return
    fi

    ndk_root=$(find "$sdk_root/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | while read -r candidate; do
        if is_valid_ndk "$candidate"; then
            printf "%s\n" "$candidate"
        fi
    done | sort -V | tail -n 1 || true)

    if [ -n "$ndk_root" ]; then
        log_warn "默认 NDK 版本 $DEFAULT_NDK_VERSION 不存在，回退到 $(basename "$ndk_root")"
        printf "%s\n" "$ndk_root"
        return
    fi

    log_error "未找到任何可用的 Android NDK"
    log_error "请在 Android Studio 的 SDK Tools 中安装 NDK (Side by side)"
    exit 1
}

ensure_java_tools() {
    if ! command -v javac >/dev/null 2>&1; then
        log_error "未找到 javac。macOS 可运行: brew install openjdk@17"
        exit 1
    fi
    require_command keytool
}

ensure_cargo_apk() {
    if ! cargo apk --version >/dev/null 2>&1; then
        log_info "安装 cargo-apk..."
        cargo install cargo-apk
    fi
}

ensure_keystore() {
    local keystore_path="${CARGO_APK_RELEASE_KEYSTORE:-${RUNNER_TEMP:-/tmp}/ale-my-eyes-android-release.keystore}"
    local keystore_password="${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-android}"

    if [ ! -f "$keystore_path" ]; then
        log_info "生成临时 Android 签名 keystore..."
        keytool -genkeypair -v \
            -storetype PKCS12 \
            -keystore "$keystore_path" \
            -storepass "$keystore_password" \
            -alias androiddebugkey \
            -keypass "$keystore_password" \
            -keyalg RSA \
            -keysize 2048 \
            -validity 10000 \
            -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1
    fi

    export CARGO_APK_RELEASE_KEYSTORE="$keystore_path"
    export CARGO_APK_RELEASE_KEYSTORE_PASSWORD="$keystore_password"
}

prepare_output_dir() {
    rm -rf "$PACKAGE_DIR"
    mkdir -p "$PACKAGE_DIR"
}

output_name_for_target() {
    case "$1" in
        aarch64-linux-android)
            printf "%s\n" "ale-my-eyes-arm64.apk"
            ;;
        armv7-linux-androideabi)
            printf "%s\n" "ale-my-eyes-armv7.apk"
            ;;
        *)
            log_error "不支持的 Android target: $1"
            exit 1
            ;;
    esac
}

clear_apk_outputs() {
    local target="$1"
    rm -f target/release/apk/*.apk "target/$target/release/apk"/*.apk 2>/dev/null || true
}

locate_apk_for_target() {
    local target="$1"
    local apk_path=""
    local target_dir="target/$target/release/apk"

    if [ -d "$target_dir" ]; then
        apk_path=$(find "$target_dir" -maxdepth 1 -type f -name "*.apk" | sort | tail -n 1 || true)
    fi

    if [ -z "$apk_path" ] && [ -d "target/release/apk" ]; then
        apk_path=$(find "target/release/apk" -maxdepth 1 -type f -name "*.apk" | sort | tail -n 1 || true)
    fi

    if [ -z "$apk_path" ] || [ ! -f "$apk_path" ]; then
        log_error "未找到 $target 构建产物 APK"
        exit 1
    fi

    printf "%s\n" "$apk_path"
}

build_target() {
    local target="$1"
    local output_name="$2"
    local apk_path=""

    clear_apk_outputs "$target"

    log_info "构建 $target..."
    ANDROID_HOME="$ANDROID_HOME" ANDROID_NDK_ROOT="$ANDROID_NDK_ROOT" cargo apk build -p ale-gui --target "$target" --lib --release

    apk_path=$(locate_apk_for_target "$target")
    cp "$apk_path" "$PACKAGE_DIR/$output_name"
    log_info "已生成: $PACKAGE_DIR/$output_name"
}

create_archive() {
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
}

print_summary() {
    log_info "Android 打包完成"
    printf "输出目录: %s\n" "$PACKAGE_DIR"

    if [ -f "$PACKAGE_DIR/ale-my-eyes-arm64.apk" ]; then
        printf "arm64 APK: %s/%s\n" "$PACKAGE_DIR" "ale-my-eyes-arm64.apk"
    fi
    if [ -f "$PACKAGE_DIR/ale-my-eyes-armv7.apk" ]; then
        printf "armv7 APK: %s/%s\n" "$PACKAGE_DIR" "ale-my-eyes-armv7.apk"
    fi
}

main() {
    local sdk_root=""
    local ndk_root=""
    local android_jar=""
    local build_tools_root=""
    local output_name=""

    require_repo_root
    require_command rustup
    require_command cargo
    require_command find
    ensure_java_tools

    sdk_root=$(resolve_sdk_root)
    ndk_root=$(resolve_ndk_root "$sdk_root")
    android_jar=$(resolve_android_jar "$sdk_root")
    build_tools_root=$(resolve_build_tools "$sdk_root")

    export ANDROID_HOME="$sdk_root"
    export ANDROID_NDK_ROOT="$ndk_root"
    export ANDROID_NDK_HOME="$ndk_root"
    export ANDROID_NDK="$ndk_root"
    export NDK_HOME="$ndk_root"
    export ANDROID_NDK_VERSION="$(basename "$ndk_root")"
    unset ANDROID_SDK_ROOT
    export PATH="$build_tools_root:$PATH"

    log_info "使用 Android SDK: $ANDROID_HOME"
    log_info "使用 Android NDK: $ANDROID_NDK_ROOT"
    log_info "使用 android.jar: $android_jar"
    log_info "使用 build-tools: $build_tools_root"

    ensure_cargo_apk

    log_info "编译 Android Java 源码..."
    bash scripts/build-android-java.sh "$android_jar"

    read -r -a BUILD_TARGETS <<< "${ANDROID_BUILD_TARGETS:-$DEFAULT_TARGETS}"
    log_info "添加 Android Rust targets..."
    rustup target add "${BUILD_TARGETS[@]}"

    ensure_keystore
    prepare_output_dir

    for target in "${BUILD_TARGETS[@]}"; do
        output_name=$(output_name_for_target "$target")
        build_target "$target" "$output_name"
    done

    create_archive
    print_summary
}

main "$@"
