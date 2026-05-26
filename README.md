# Ale, My Eyes! - 智能视觉辅助系统

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Made%20with-Rust-red.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Android-blue.svg)]()

> 对着摄像头或屏幕说话，AI 用自然语言回答你的问题，还能帮你操作电脑

## 项目简介

**Ale, My Eyes!** 是一个基于 Rust 的跨平台智能视觉辅助系统。用户通过语音向设备提问，AI 结合摄像头画面或屏幕截图给出自然语言回答，并可在桌面端自动执行键鼠操作。

**两种使用模式：**

| 平台 | 交互方式 |
|------|----------|
| **Android** | 打开即是相机界面，持续监听语音，对着摄像头提问，AI 实时回答 |
| **PC/Linux** | 小窗口常驻，持续监控屏幕，语音下达指令，AI 自动操作键鼠 |

## 功能特性

### 语音交互
- **持续监听** — 应用启动即开始录音，VAD 自动检测说话结束并触发处理
- **语音活动检测** — 基于能量的 VAD 状态机（静默 → 说话中 → 说话结束）
- **多语言识别** — 本地支持 17 种语言 + 自动检测，云端支持 100+ 种
- **语音合成** — AI 回答自动朗读，高风险操作语音解释并等待确认

### 视觉理解
- **视觉问答** — 对摄像头画面或屏幕截图提问，AI 用自然语言回答
- **上下文管理** — 自动维护对话历史、视觉记忆和长期记忆，智能压缩
- **按需截帧** — 说话时截取当前画面，连同语音一起发送给 AI

### 桌面自动化
- **全功能键鼠控制** — 点击、滚动、打字、快捷键、打开/关闭应用、文件操作
- **风险分级** — 低风险自动执行，高风险语音解释原因后等待用户确认
- **结构化操作** — AI 通过 Function Calling 返回可执行的操作指令

### 跨平台
- **云端 + 本地** — 复杂任务走云端 API，简单任务本地离线处理
- **自适应推理** — 根据设备性能和网络状态自动选择最佳推理方式
- **共享代码** — 桌面和 Android 共享 Rust 核心库 + Slint UI

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

# 命令行
cargo run -p ale-cli -- transcribe --audio input.wav
```

## 技术架构

### 项目结构

```
ale-my-eyes-rust/
├── ale-core/                  # 核心库
│   ├── src/
│   │   ├── lib.rs             # AleEngine 主入口
│   │   ├── cloud.rs           # 云端 API（OpenAI 兼容 + Function Calling）
│   │   ├── inference.rs       # 自适应推理引擎
│   │   ├── vad.rs             # 语音活动检测（VAD）
│   │   ├── actions.rs         # 操作指令协议（11 种操作 + 风险分级）
│   │   ├── context.rs         # 上下文管理（对话/视觉/长期记忆）
│   │   ├── config.rs          # 配置系统
│   │   ├── asr.rs             # 本地 ASR（whisper-rs）
│   │   ├── vlm.rs             # 本地 VLM
│   │   ├── llm.rs             # 本地 LLM
│   │   ├── tts.rs             # 系统 TTS
│   │   ├── downloader.rs      # 模型下载器
│   │   └── manager.rs         # 模型管理器
│   └── Cargo.toml
├── ale-cli/                   # 命令行工具
├── ale-gui/                   # 跨平台 GUI (Slint)
│   ├── ui/
│   │   ├── app.slint          # 主窗口 + 导航
│   │   ├── main-screen.slint  # 移动端主界面
│   │   ├── desktop-screen.slint # 桌面浮动窗口
│   │   ├── settings-screen.slint
│   │   ├── diagnostics-screen.slint
│   │   └── widgets.slint      # 通用组件库
│   └── src/
│       ├── lib.rs             # 共享逻辑 + AppState
│       ├── main.rs            # 桌面入口
│       ├── android.rs         # Android 入口
│       ├── audio.rs           # 录音（cpal / oboe）
│       ├── camera.rs          # Android 相机（JNI Camera2）
│       ├── screen_capture.rs  # 桌面屏幕截图（xcap）
│       ├── automation.rs      # 桌面键鼠自动化（enigo）
│       ├── file_picker.rs     # 文件选择
│       └── tts_player.rs      # TTS 播放
├── scripts/                   # 构建/打包脚本
└── Cargo.toml
```

### 核心模块

| 模块 | 文件 | 功能 |
|------|------|------|
| VAD | `ale-core/src/vad.rs` | 能量检测 + 状态机，自适应阈值 |
| Actions | `ale-core/src/actions.rs` | 11 种操作类型，3 级风险评估 |
| Context | `ale-core/src/context.rs` | 对话历史 + 视觉记忆 + 长期记忆，自动压缩 |
| Vision API | `ale-core/src/cloud.rs` | 自定义问题 + Function Calling |
| Screen Capture | `ale-gui/src/screen_capture.rs` | 持续/按需截图，缩放，JPEG 编码 |
| Automation | `ale-gui/src/automation.rs` | 鼠标/键盘/文件/URL/应用操作 |
| Camera | `ale-gui/src/camera.rs` | Android Camera2 JNI + YUV→RGBA |

### 技术栈

| 层级 | 技术 |
|------|------|
| GUI 框架 | Slint 1.16（跨平台声明式 UI） |
| 本地 ASR | whisper-rs 0.16（whisper.cpp FFI） |
| 云端 ASR | OpenAI Whisper API |
| 视觉理解 | OpenAI GPT-4o Vision + Function Calling |
| 桌面截屏 | xcap 0.9（X11/Wayland/Windows/macOS） |
| 键鼠控制 | enigo 0.6（X11/Wayland/Windows/macOS） |
| Android 相机 | JNI Camera2 API |
| 桌面音频 | cpal + rodio |
| Android 音频 | oboe + JNI MediaPlayer |

### 数据流

```
┌─────────────────────────────────────────────────────────┐
│  Android: 相机帧 + 语音 ──┐                              │
│                           ├→ 云端 API (GPT-4o)          │
│  Desktop: 屏幕截图 + 语音 ─┘   ├→ 语音转文字 (Whisper)   │
│                                ├→ 视觉问答 (Vision)      │
│                                └→ 操作指令 (Function Call)│
│                                         │                │
│                    ┌────────────────────┘                │
│                    ▼                                      │
│  Android: TTS 朗读回答 + 显示文字                         │
│  Desktop:  TTS 朗读 + 执行键鼠操作（高风险需确认）        │
└─────────────────────────────────────────────────────────┘
```

## 相关项目

- **[ale-server](https://github.com/Risaly-Noroki-Dev-Club/ale-server)** — HTTP API 服务器（独立项目）

## 开发指南

### 环境要求

- **Rust**: 1.70.0+
- **系统依赖**:
  - Linux: `libasound2-dev libfontconfig-dev libspeechd-dev libpipewire-0.3-dev libwayland-dev libxrandr-dev libdbus-1-dev libegl-dev libgbm-dev libxcb-shape0-dev libxcb-xfixes0-dev libclang-dev`
  - Windows: Visual Studio Build Tools
  - Android: Android NDK 25+, `cargo-apk`

### 构建

```bash
git clone https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes.git
cd ale-my-eyes

# 桌面构建
cargo build --release -p ale-gui

# 带本地推理支持
cargo build --release -p ale-core --features local-inference

# Android 构建
export ANDROID_NDK_ROOT=/path/to/android-ndk
./scripts/package-android.sh
```

### 常用命令

```bash
cargo check --workspace                    # 检查整个 workspace
cargo fmt && cargo clippy --workspace      # 格式化 + lint
cargo test -p ale-core                     # 运行核心库测试
cargo run -p ale-gui                       # 启动桌面 GUI
```

### 发布

GitHub Actions 自动构建。推送 `v*` 标签或手动触发 workflow 会发布：

- `ale-my-eyes-windows.exe` (Windows)
- `ale-my-eyes_0.1.0_amd64.deb` (Linux)
- `ale-my-eyes-android.apk` (Android)

## 许可证

MIT License - 查看 [LICENSE](LICENSE)

## 致谢

- [whisper.cpp](https://github.com/ggml-org/whisper.cpp) - 本地语音识别引擎
- [whisper-rs](https://github.com/tazz4843/whisper-rs) - Rust FFI 绑定
- [OpenAI](https://openai.com/) - 云端 API
- [Slint](https://slint.dev/) - 跨平台 UI 框架
- [xcap](https://github.com/nashaofu/xcap) - 跨平台屏幕截图
- [enigo](https://github.com/enigo-rs/enigo) - 键鼠自动化
- [水素&lin] - 最初的动力

## 联系

- **项目主页**: [github.com/Risaly-Noroki-Dev-Club/ale-my-eyes](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes)
- **问题反馈**: [GitHub Issues](https://github.com/Risaly-Noroki-Dev-Club/ale-my-eyes/issues)
- **邮箱**: erika@risnordev.org
