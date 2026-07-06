#!/bin/bash
# Ale, My Eyes! macOS 打包脚本
# 用法: ./scripts/package-macos.sh

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}开始打包 Ale, My Eyes! macOS 版本...${NC}"

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

# 检查 macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo -e "${RED}错误: 此脚本只能在 macOS 上运行${NC}"
    exit 1
fi

# 构建 macOS 版本
echo -e "${YELLOW}构建 macOS 版本...${NC}"
cargo build --release

# 检查构建产物
if [ ! -f "target/release/ale-gui" ]; then
    echo -e "${RED}错误: 构建失败，未找到 ale-gui${NC}"
    exit 1
fi

# 创建 .app 结构
APP_NAME="Ale, My Eyes!"
APP_DIR="${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
CONFIG_DIR="${RESOURCES_DIR}/config"

echo -e "${YELLOW}创建 .app 结构...${NC}"
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${CONFIG_DIR}"

# 复制可执行文件
echo -e "${YELLOW}复制可执行文件...${NC}"
cp target/release/ale-gui "${MACOS_DIR}/"
cp target/release/ale-cli "${MACOS_DIR}/"

# 设置执行权限
chmod +x "${MACOS_DIR}/ale-gui"
chmod +x "${MACOS_DIR}/ale-cli"

# 创建 Info.plist
echo -e "${YELLOW}创建 Info.plist...${NC}"
cat > "${CONTENTS_DIR}/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>zh_CN</string>
    <key>CFBundleDisplayName</key>
    <string>Ale, My Eyes!</string>
    <key>CFBundleExecutable</key>
    <string>ale-gui</string>
    <key>CFBundleIdentifier</key>
    <string>com.alemyeyes.app</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>AleMyEyes</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticTermination</key>
    <false/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Ale, My Eyes! 需要使用麦克风来接收您的语音指令。</string>
    <key>NSScreenCaptureUsageDescription</key>
    <string>Ale, My Eyes! 需要屏幕录制权限来分析屏幕内容并回答您的问题。</string>
    <key>NSCameraUsageDescription</key>
    <string>Ale, My Eyes! 需要使用摄像头来捕获视频画面。</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
</dict>
</plist>
EOF

# 创建 PkgInfo 文件
echo -e "${YELLOW}创建 PkgInfo...${NC}"
echo -n "APPL????" > "${CONTENTS_DIR}/PkgInfo"

# 创建默认配置文件
echo -e "${YELLOW}创建默认配置文件...${NC}"
cat > "${CONFIG_DIR}/config.json" << 'EOF'
{
  "cloud_api": {
    "provider": "openai",
    "api_key": "",
    "api_url": "https://api.openai.com/v1",
    "model": "gpt-4o",
    "max_tokens": 1024,
    "timeout": 30
  },
  "models": {
    "auto_download": true,
    "max_download_size": 524288000,
    "preferred_quality": "balanced",
    "offline_mode": false,
    "models_dir": "models"
  },
  "inference": {
    "mode": "adaptive",
    "prefer_cloud": true,
    "timeout": 30,
    "fallback_to_local": true
  },
  "audio": {
    "sample_rate": 16000,
    "channels": 1,
    "buffer_size": 4096,
    "voice": "default",
    "speed": 1.0
  },
  "ui": {
    "language": "zh-CN",
    "theme": "system",
    "font_size": 16,
    "high_contrast": false,
    "screen_reader": true,
    "auto_speak": true
  }
}
EOF

# 创建图标文件（占位符）
echo -e "${YELLOW}创建图标文件...${NC}"
# 如果有 icon.icfs 文件，使用它；否则创建占位符
if [ -f "assets/icon.icns" ]; then
    cp "assets/icon.icns" "${RESOURCES_DIR}/icon.icns"
    echo -e "${GREEN}已复制图标文件${NC}"
else
    # 创建一个简单的占位符图标（实际项目应该有真实图标）
    echo "图标占位符 - 请替换为真实的 icon.icns 文件" > "${RESOURCES_DIR}/icon.txt"
    echo -e "${YELLOW}警告: 未找到 icon.icns，请手动添加${NC}"
fi

# 创建 README
echo -e "${YELLOW}创建 README...${NC}"
cat > "${CONTENTS_DIR}/README.md" << 'EOF'
# Ale, My Eyes! - 智能视觉辅助系统

对着摄像头或屏幕说话，AI 用自然语言回答你的问题，还能帮你操作电脑。

## 快速开始

### 1. 配置 API 密钥
运行应用后，点击 ⚙ 图标打开设置，输入您的 OpenAI API 密钥。

或者编辑配置文件：
```
~/Library/Application Support/ale-my-eyes/config.json
```

### 2. 启动应用
双击 Ale, My Eyes!.app 启动应用。

## 功能特性

- 语音交互：启动即监听，VAD 自动检测说话结束并处理
- 视觉问答：对屏幕提问，AI 结合画面自然语言回答
- 桌面自动化：语音控制键鼠操作
- 多语言识别：支持 17 种语言
- macOS 原生支持

## 系统要求

- macOS 10.15 (Catalina) 或更高版本
- 至少 4GB 内存
- 麦克风和扬声器
- 网络连接（用于云端 API）

## 权限说明

首次运行时，应用会请求以下权限：
- **麦克风**：用于语音输入
- **屏幕录制**：用于屏幕捕获
- **摄像头**：用于视频捕获（可选）

请在系统设置 > 隐私与安全性中授予权限。

## 获取帮助

如需帮助，请访问项目主页或提交 Issue。
EOF

# Ad-hoc 代码签名（macOS 必需）
echo -e "${YELLOW}Ad-hoc 代码签名...${NC}"
codesign --force --deep --sign - "${APP_DIR}" 2>/dev/null || {
    echo -e "${YELLOW}签名失败，尝试逐个签名...${NC}"
    codesign --force --sign - "${MACOS_DIR}/ale-gui" 2>/dev/null || true
    codesign --force --sign - "${MACOS_DIR}/ale-cli" 2>/dev/null || true
    codesign --force --deep --sign - "${APP_DIR}" 2>/dev/null || true
}
echo -e "${GREEN}签名完成${NC}"

# 创建 DMG 安装包（可选）
echo -e "${YELLOW}是否创建 DMG 安装包？(y/n)${NC}"
read -r create_dmg

if [[ "$create_dmg" =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}创建 DMG 安装包...${NC}"

    # 创建临时目录
    DMG_DIR="dmg_temp"
    rm -rf "${DMG_DIR}"
    mkdir -p "${DMG_DIR}"

    # 复制 .app 到临时目录
    cp -R "${APP_DIR}" "${DMG_DIR}/"

    # 创建应用程序链接
    ln -s /Applications "${DMG_DIR}/Applications"

    # 创建 DMG
    DMG_NAME="AleMyEyes-$(date +%Y%m%d).dmg"
    hdiutil create -volname "Ale, My Eyes!" -srcfolder "${DMG_DIR}" -ov -format UDZO "${DMG_NAME}"

    # 清理临时目录
    rm -rf "${DMG_DIR}"

    echo -e "${GREEN}DMG 创建完成: ${DMG_NAME}${NC}"
fi

echo -e "${GREEN}打包完成！${NC}"
echo -e "${GREEN}输出: ${APP_DIR}${NC}"
echo ""
echo -e "${YELLOW}使用说明：${NC}"
echo -e "1. 双击 ${APP_DIR} 启动应用"
echo -e "2. 首次运行需要配置 API 密钥"
echo -e "3. 在系统设置中授予必要的权限"
echo ""
echo -e "${YELLOW}如需创建 DMG 安装包，请运行：${NC}"
echo -e "  ./scripts/package-macos.sh"
