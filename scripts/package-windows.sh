#!/bin/bash
# Ale, My Eyes! Windows 打包脚本
# 用法: ./scripts/package-windows.sh

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}开始打包 Ale, My Eyes! Windows 版本...${NC}"

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

# 添加 Windows 目标
echo -e "${YELLOW}添加 Windows 目标...${NC}"
rustup target add x86_64-pc-windows-msvc

# 构建 Windows 版本
echo -e "${YELLOW}构建 Windows 版本...${NC}"
cargo build --release --target x86_64-pc-windows-msvc

# 创建打包目录
PACKAGE_DIR="ale-my-eyes-windows"
echo -e "${YELLOW}创建打包目录: ${PACKAGE_DIR}${NC}"
rm -rf "${PACKAGE_DIR}"
mkdir -p "${PACKAGE_DIR}"

# 复制可执行文件
echo -e "${YELLOW}复制可执行文件...${NC}"
cp target/x86_64-pc-windows-msvc/release/ale-cli.exe "${PACKAGE_DIR}/"
cp target/x86_64-pc-windows-msvc/release/ale-gui.exe "${PACKAGE_DIR}/"

# 创建模型目录
echo -e "${YELLOW}创建模型目录...${NC}"
mkdir -p "${PACKAGE_DIR}/models"
mkdir -p "${PACKAGE_DIR}/models/bundled"
mkdir -p "${PACKAGE_DIR}/models/downloaded"

# 创建配置目录
echo -e "${YELLOW}创建配置目录...${NC}"
mkdir -p "${PACKAGE_DIR}/config"

# 创建默认配置文件
echo -e "${YELLOW}创建默认配置文件...${NC}"
cat > "${PACKAGE_DIR}/config/config.json" << EOF
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

# 创建启动脚本
echo -e "${YELLOW}创建启动脚本...${NC}"
cat > "${PACKAGE_DIR}/start-gui.bat" << EOF
@echo off
echo 启动 Ale, My Eyes! 图形界面...
ale-gui.exe
pause
EOF

# 创建 README
echo -e "${YELLOW}创建 README...${NC}"
cat > "${PACKAGE_DIR}/README.txt" << EOF
Ale, My Eyes! - 智能视觉辅助系统
================================

对着摄像头或屏幕说话，AI 用自然语言回答你的问题，还能帮你操作电脑。

快速开始
--------

1. 配置 API 密钥
   打开 config/config.json 文件，设置您的 OpenAI API 密钥：
   "api_key": "sk-your-api-key-here"

2. 启动图形界面
   双击 start-gui.bat 启动图形界面

功能特性
--------

- 语音交互：启动即监听，VAD 自动检测说话结束并处理
- 视觉问答：对屏幕提问，AI 结合画面自然语言回答
- 桌面自动化：语音控制键鼠操作
- 多语言识别：支持 17 种语言
- 平台原生样式：Windows Fluent / Android Material 3

HTTP API 服务器
---------------

如需 HTTP API 服务，请单独安装 ale-server：
https://github.com/Risaly-Noroki-Dev-Club/ale-server

系统要求
--------

- Windows 10 或更高版本
- 至少 4GB 内存
- 麦克风和扬声器
- 网络连接（用于云端 API）

配置说明
--------

配置文件位于 config/config.json，包含以下设置：

- cloud_api: 云端 API 配置
- models: 模型下载和管理设置
- inference: 推理模式设置
- audio: 音频设置
- ui: 界面设置

获取帮助
--------

如需帮助，请访问项目主页或提交 Issue。

EOF

# 创建压缩包
echo -e "${YELLOW}创建压缩包...${NC}"
if command -v 7z &> /dev/null; then
    7z a "ale-my-eyes-windows.zip" "${PACKAGE_DIR}"
elif command -v zip &> /dev/null; then
    zip -r "ale-my-eyes-windows.zip" "${PACKAGE_DIR}"
else
    echo -e "${YELLOW}警告: 未找到 7z 或 zip，跳过压缩包创建${NC}"
fi

echo -e "${GREEN}打包完成！${NC}"
echo -e "${GREEN}输出目录: ${PACKAGE_DIR}${NC}"
if [ -f "ale-my-eyes-windows.zip" ]; then
    echo -e "${GREEN}压缩包: ale-my-eyes-windows.zip${NC}"
fi

echo -e "${YELLOW}下一步：${NC}"
echo -e "1. 将 ${PACKAGE_DIR} 目录复制到目标 Windows 机器"
echo -e "2. 编辑 config/config.json 设置 API 密钥"
echo -e "3. 运行 start-server.bat 启动服务器"
echo -e "4. 运行 start-gui.bat 启动图形界面"