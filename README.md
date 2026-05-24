# Ale, My Eyes! - 智能辅助系统

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Made%20with-Rust-red.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Android-blue.svg)]()

> 为视障人士打造的智能辅助系统，使用 VLM/ASR/TTS 技术帮助用户更好地使用电脑

## 项目简介

**Ale, My Eyes!** 是一个基于 Rust 开发的跨平台智能辅助系统，专为视障人士设计。通过集成视觉语言模型（VLM）、语音识别（ASR）和语音合成（TTS）技术，为用户提供自然的语音交互体验。

- **低性能设备友好**：优化内存使用，支持联发科/赛扬等低端CPU
- **云端优先**：复杂任务使用云端API，本地处理简单任务
- **智能切换**：根据设备性能和网络状态自动选择最佳推理方式
- **跨平台**：桌面（Windows/Linux）和 Android 共享同一套 Rust 代码和 Slint UI

## 功能特性

- **语音识别**：麦克风录音，上传云端 Whisper 转写，支持中英文等多语言
- **图像描述**：选择图片，调用 GPT-4o Vision 获取描述
- **语音合成**：朗读识别结果和状态信息
- **自适应推理**：根据设备性能自动选择本地或云端推理
- **离线支持**：无网络时降级到本地模型（开发中）

## 快速开始

### 下载安装

从 [Releases](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes/releases) 页面下载：

- **Windows**: `ale-my-eyes-windows.exe`
- **Linux**: `ale-my-eyes_0.1.0_amd64.deb`
- **Android**: `ale-my-eyes-android.apk`

### 配置 API 密钥

1. 打开应用，进入 **设置** 页面
2. 填写 API Key（OpenAI 或兼容接口）
3. API URL 默认 `https://api.openai.com/v1`，可改为 OpenRouter、Azure 等
4. 点击 **测试连接** 验证配置

### 启动使用

```bash
# 桌面 GUI
cargo run -p ale-gui

# HTTP 服务器
cargo run -p ale-server

# 命令行
cargo run -p ale-cli -- transcribe --audio input.wav
```

## 技术架构

### 项目结构

```
ale-my-eyes-rust/
├── ale-core/              # 核心库（引擎、配置、云端API、推理）
├── ale-server/            # HTTP 服务器 (Axum)
├── ale-cli/               # 命令行工具
├── ale-gui/               # 跨平台 GUI (Slint)
│   ├── ui/                # Slint UI 定义文件
│   │   ├── app.slint      # 主窗口、底部导航栏
│   │   ├── main-screen.slint
│   │   ├── settings-screen.slint
│   │   ├── diagnostics-screen.slint
│   │   └── widgets.slint  # 复用组件
│   └── src/               # Rust 逻辑
│       ├── lib.rs         # 共享逻辑、Slint 回调
│       ├── main.rs        # 桌面入口
│       ├── android.rs     # Android 入口
│       ├── audio.rs       # 录音（cpal/oboe）
│       ├── file_picker.rs # 文件选择（rfd/JNI）
│       └── tts_player.rs  # TTS 播放（rodio/JNI）
├── scripts/               # 构建脚本
└── Cargo.toml
```

### 技术栈

| 层级 | 技术 |
|------|------|
| GUI 框架 | Slint 1.16（跨平台声明式 UI） |
| 语音识别 | OpenAI Whisper API (cloud) / whisper-rs (local) |
| 语音合成 | OpenAI TTS API (cloud) / Piper (local) |
| 视觉理解 | OpenAI GPT-4o Vision |
| 桌面音频 | cpal + rodio |
| Android 音频 | oboe + JNI MediaPlayer |
| Web 框架 | Axum + Tokio |

### 推理策略

```
用户设备检测
├── 低端设备 → 云端 API（默认）
├── 中端设备 → 根据任务复杂度自动切换
└── 高端设备 → 优先使用高质量模型
```

## 开发指南

### 环境要求

- **Rust**: 1.70.0+
- **系统依赖**:
  - Linux: `libasound2-dev`, `libfontconfig-dev`
  - Windows: Visual Studio Build Tools
  - Android: Android NDK 25+, `cargo-apk`

### 构建

```bash
git clone https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes.git
cd ale-my-eyes

# 桌面构建
cargo build --release -p ale-gui

# Android 构建
export ANDROID_NDK_ROOT=/path/to/android-ndk
./scripts/package-android.sh
```

### 常用命令

```bash
cargo check --workspace          # 检查整个 workspace
cargo fmt && cargo clippy --workspace  # 格式化 + lint
cargo test                       # 运行测试
cargo run -p ale-gui             # 启动桌面 GUI
```

### 发布

GitHub Actions 自动构建。推送 `v*` 标签或手动触发 workflow 会发布：

- `ale-my-eyes-windows.exe` (Windows)
- `ale-my-eyes_0.1.0_amd64.deb` (Linux)
- `ale-my-eyes-android.apk` (Android)

## CLI 用法

```bash
cargo run -p ale-cli -- transcribe --audio input.wav
cargo run -p ale-cli -- synthesize --text "你好" --output speech.wav
cargo run -p ale-cli -- describe --image screenshot.png
cargo run -p ale-cli -- test-connection
cargo run -p ale-cli -- status
```

## HTTP Server

```bash
cargo run -p ale-server
```

默认 `0.0.0.0:8000`：

- `GET /health` - 健康检查
- `POST /asr/transcribe` - 语音识别
- `POST /tts/synthesize` - 语音合成
- `POST /vlm/describe` - 图片描述

## 许可证

MIT License - 查看 [LICENSE](LICENSE)

## 致谢

- [Whisper](https://github.com/openai/whisper) - 语音识别
- [Piper](https://github.com/rhasspy/piper) - 语音合成
- [OpenAI](https://openai.com/) - 云端 API
- [Slint](https://slint.dev/) - 跨平台 UI 框架
- [Axum](https://github.com/tokio-rs/axum) - Web 框架
- [水素&lin] - 最初的动力

## 联系

- **项目主页**: [github.com/Risaly-Noroki-Dev-Club/ale-my-eyes](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes)
- **问题反馈**: [GitHub Issues](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes/issues)
- **邮箱**: erika@risnordev.org
