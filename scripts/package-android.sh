#!/bin/bash
# Ale, My Eyes! Android 打包脚本 (Slint 版本)
# 用法: ./scripts/package-android.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}开始打包 Ale, My Eyes! Android 版本...${NC}"

if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}错误: 请在项目根目录运行此脚本${NC}"
    exit 1
fi

if ! command -v rustup &> /dev/null; then
    echo -e "${RED}错误: 未找到 rustup，请先安装 Rust${NC}"
    exit 1
fi

# Install cargo-apk if missing
if ! command -v cargo-apk &> /dev/null; then
    echo -e "${YELLOW}安装 cargo-apk...${NC}"
    cargo install cargo-apk
fi

# Check Android NDK
if [ -z "$ANDROID_NDK_ROOT" ]; then
    echo -e "${RED}错误: 未设置 ANDROID_NDK_ROOT 环境变量${NC}"
    echo -e "${YELLOW}请安装 Android NDK 并设置环境变量：${NC}"
    echo -e "export ANDROID_NDK_ROOT=/path/to/android-ndk"
    exit 1
fi

# Add Android targets
echo -e "${YELLOW}添加 Android 目标...${NC}"
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi

# Build arm64 release APK
echo -e "${GREEN}构建 arm64 版本...${NC}"
cargo apk build -p ale-gui --target aarch64-linux-android --lib --release 2>&1

# Build armv7 release APK
echo -e "${GREEN}构建 armv7 版本...${NC}"
cargo apk build -p ale-gui --target armv7-linux-androideabi --lib --release 2>&1

# Copy APKs to output directory
PACKAGE_DIR="ale-my-eyes-android"
echo -e "${YELLOW}创建输出目录: ${PACKAGE_DIR}${NC}"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}"

APK_ARM64="target/aarch64-linux-android/release/apk/ale-gui.apk"
APK_ARMV7="target/armv7-linux-androideabi/release/apk/ale-gui.apk"

if [ -f "$APK_ARM64" ]; then
    cp "$APK_ARM64" "${PACKAGE_DIR}/ale-my-eyes-arm64.apk"
    echo -e "${GREEN}arm64 APK: ${PACKAGE_DIR}/ale-my-eyes-arm64.apk${NC}"
fi

if [ -f "$APK_ARMV7" ]; then
    cp "$APK_ARMV7" "${PACKAGE_DIR}/ale-my-eyes-armv7.apk"
    echo -e "${GREEN}armv7 APK: ${PACKAGE_DIR}/ale-my-eyes-armv7.apk${NC}"
fi

# Create archive
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
echo -e "1. 将 APK 传输到 Android 设备"
echo -e "2. 在设备上安装 APK（需要允许未知来源）"
echo -e "3. 或使用 adb install: adb install ${PACKAGE_DIR}/ale-my-eyes-arm64.apk"
