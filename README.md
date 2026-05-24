# Ale, My Eyes! - 智能辅助系统

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Made%20with-Rust-red.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Android-blue.svg)]()

> 为视障人士打造的智能辅助系统，使用 VLM/ASR/TTS 技术帮助用户更好地使用电脑

## 📖 项目简介

**Ale, My Eyes!** 是一个基于 Rust 开发的跨平台智能辅助系统，专为视障人士设计。通过集成视觉语言模型（VLM）、语音识别（ASR）和语音合成（TTS）技术，为用户提供自然的语音交互体验，替代传统的 VDA 和讲述人工具。

### 🎯 核心理念

- **低性能设备友好**：优化内存使用，支持联发科/赛扬等低端CPU
- **云端优先**：复杂任务使用云端API，本地处理简单任务
- **智能切换**：根据设备性能和网络状态自动选择最佳推理方式
- **离线可用**：无网络时降级到本地模型，保证基本功能可用

## ✨ 功能特性

### 🎤 语音交互
- **语音识别**：通过麦克风输入语音指令，支持中英文等多语言
- **语音合成**：系统状态和屏幕内容的语音反馈，支持多种语音风格
- **自然语言理解**：支持自然语言指令，如"打开浏览器"、"读取当前页面"

### 👁️ 视觉理解
- **屏幕内容分析**：理解屏幕上的文字、按钮、图标等元素
- **图像描述**：上传图像获取详细描述，支持拍照识别
- **界面元素识别**：识别UI控件并提供语音导航

### 🤖 智能推理
- **自适应推理**：根据设备性能自动选择本地或云端推理
- **离线支持**：无网络时使用本地轻量级模型
- **模型管理**：自动下载、更新、清理模型文件

### 🔧 配置管理
- **用户偏好设置**：语言、主题、字体大小等个性化配置
- **云端API配置**：支持 OpenAI、Anthropic 等多种云端服务
- **推理模式选择**：本地/云端/自适应三种模式可选

## 🚀 快速开始

### 📦 下载安装

从 [Releases](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes/releases) 页面下载适合您平台的安装包：

- **Windows**: `ale-my-eyes-windows.zip`
- **Linux**: `ale-my-eyes-linux.tar.gz`
- **Android**: `ale-my-eyes-android.apk`

### ⚙️ 配置 API 密钥

1. 打开配置文件 `config/config.json`
2. 设置您的 OpenAI API 密钥：

```json
{
  "cloud_api": {
    "provider": "openai",
    "api_key": "sk-your-api-key-here",
    "api_url": "https://api.openai.com/v1",
    "model": "gpt-4o"
  }
}
```

### 🎮 启动使用

#### Windows
```cmd
# 启动服务器
start-server.bat

# 启动图形界面
start-gui.bat
```

#### Linux
```bash
# 启动服务器
./start-server.sh

# 启动图形界面
./start-gui.sh
```

#### Android
1. 安装 APK 文件
2. 打开应用，输入 API 密钥
3. 开始使用语音交互功能

## 🏗️ 技术架构

### 📁 项目结构

```
ale-my-eyes/
├── ale-core/                    # 核心库
│   ├── cloud.rs                # 云端API集成
│   ├── inference.rs            # 推理引擎
│   ├── downloader.rs           # 模型下载器
│   ├── manager.rs              # 模型管理器
│   └── config.rs               # 配置系统
├── ale-server/                  # 后端服务器
├── ale-cli/                     # 命令行工具
├── ale-gui/                     # 图形界面
├── scripts/                     # 构建脚本
│   ├── package-windows.sh      # Windows 打包
│   ├── package-linux.sh        # Linux 打包
│   └── package-android.sh      # Android 打包
└── Cargo.toml                   # 项目配置
```

### 🔧 技术栈

- **后端**: Rust + Axum + Tokio
- **语音识别**: Whisper (本地) + OpenAI Whisper API (云端)
- **语音合成**: Piper TTS (本地) + OpenAI TTS API (云端)
- **视觉理解**: OpenAI GPT-4o Vision API
- **GUI**: iced (跨平台桌面应用)
- **构建**: Cargo + cargo-ndk (Android)

### 📊 推理策略

```
用户设备检测
├── 低端设备 (联发科/赛扬)
│   ├── 网络良好 → 云端API (GPT-4o)
│   └── 网络差/离线 → 本地轻量模型
├── 中端设备 (i5/Ryzen5)
│   └── 智能选择 → 根据任务复杂度切换
└── 高端设备 (i7/Ryzen7)
    └── 优先使用高质量模型
```

## 🛠️ 开发指南

### 📋 环境要求

- **Rust**: 1.70.0 或更高版本
- **Cargo**: 包管理器
- **系统依赖**:
  - Windows: Visual Studio Build Tools
  - Linux: `libspeechd-dev`, `libasound2-dev`
  - Android: Android NDK 25+

### 🔨 从源码构建

#### 克隆仓库
```bash
git clone https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes.git
cd ale-my-eyes
```

#### 构建 Windows 版本
```bash
./scripts/package-windows.sh
```

#### 构建 Linux 版本
```bash
./scripts/package-linux.sh
```

#### 构建 Android 版本
```bash
# 设置 Android NDK 路径
export ANDROID_NDK_ROOT=/path/to/android-ndk

# 运行打包脚本
./scripts/package-android.sh
```

### 🧪 运行测试
```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test -p ale-core
```

### 📝 代码格式化
```bash
# 格式化代码
cargo fmt

# 检查代码风格
cargo clippy
```

### ✅ 常用验证命令
```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

### 🚢 云端发布产物

GitHub Actions 是发布产物的来源。推送 `v*` 标签或手动运行 `Build and Release` workflow 会构建并发布三种文件到 GitHub Release：

- Ubuntu: `ale-my-eyes_0.1.0_amd64.deb`
- Windows: `ale-my-eyes-windows.exe`
- Android: `ale-my-eyes-android.apk`

普通 push / pull request 只运行格式化和 workspace 检查，不创建 Release。

### 🖥️ 桌面 GUI

```bash
cargo run -p ale-gui
```

GUI 支持：

- 在设置页保存 API Key、API URL、模型名、语言和字体大小。
- 支持任何 OpenAI 兼容 API（如 OpenRouter、自建代理、Azure OpenAI 等），只需修改 API URL。
- 测试云端连接。
- 选择图片并调用 VLM 描述。
- 录音最多 60 秒，停止后转为 WAV 并调用 ASR 转写。
- 朗读当前结果。

Linux 构建/运行 GUI 录音功能需要系统依赖：

```bash
sudo apt-get install libasound2-dev
```

### 🧰 CLI 用法

```bash
# 语音识别，输出到终端
cargo run -p ale-cli -- transcribe --audio input.wav

# 语音识别，写入文本文件
cargo run -p ale-cli -- transcribe --audio input.wav --output transcript.txt

# 语音合成，写出 WAV 音频
cargo run -p ale-cli -- synthesize --text "你好" --output speech.wav

# 图片描述，输出到终端
cargo run -p ale-cli -- describe --image screenshot.png

# 测试云端连接
cargo run -p ale-cli -- test-connection

# 查看引擎状态
cargo run -p ale-cli -- status
```

CLI 使用与 GUI 相同的用户配置文件。请先通过 GUI 设置页保存 API Key，或手动编辑用户配置目录下的 `ale-my-eyes/config.json`。

### 🌐 HTTP Server

```bash
cargo run -p ale-server
```

默认监听 `0.0.0.0:8000`，提供：

- `GET /health` - 健康检查
- `GET /status` - 引擎详细状态
- `GET /models` - 已下载模型列表
- `POST /asr/transcribe` - 语音识别
- `POST /tts/synthesize` - 语音合成
- `POST /vlm/describe` - 图片描述

接口详情见 [`docs/API.md`](docs/API.md)。

## 📚 使用教程

### 🎤 基础语音交互

#### 1. 语音识别
```rust
use ale_core::AleEngineFactory;

let engine = AleEngineFactory::create_default().await?;

// 录制音频并识别
let audio_data = record_audio()?;
let text = engine.transcribe(&audio_data).await?;
println!("识别结果: {}", text);
```

#### 2. 语音合成
```rust
// 将文本转换为语音
let audio = engine.synthesize("欢迎使用 Ale, My Eyes!").await?;
play_audio(audio)?;
```

#### 3. 图像描述
```rust
// 描述屏幕内容或上传的图像
let image_data = capture_screen()?;
let description = engine.describe_image(&image_data).await?;
println!("图像描述: {}", description);
```

### 🔧 高级配置

#### 自定义推理模式
```json
{
  "inference": {
    "mode": "adaptive",  // "local", "cloud", "adaptive"
    "prefer_cloud": true,
    "timeout": 30,
    "fallback_to_local": true
  }
}
```

#### 模型管理配置
```json
{
  "models": {
    "auto_download": true,
    "max_download_size": 524288000,  // 500MB
    "preferred_quality": "balanced",  // "low", "balanced", "high"
    "offline_mode": false
  }
}
```


## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情

## 🙏 致谢

- [Whisper](https://github.com/openai/whisper) - 语音识别模型
- [Piper](https://github.com/rhasspy/piper) - 语音合成模型
- [OpenAI](https://openai.com/) - 云端API服务
- [iced](https://github.com/iced-rs/iced) - 跨平台GUI框架
- [Axum](https://github.com/tokio-rs/axum) - Web框架
- [水素&lin] - 最初的动力
## 📞 联系我们

- **项目主页**: [https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes)
- **问题反馈**: [GitHub Issues](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes/issues)
- **邮箱**: erika@risnordev.org
