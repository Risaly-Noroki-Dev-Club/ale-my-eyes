#!/bin/bash
# 创建发布包（无需编译）

set -e

echo "创建 Ale, My Eyes! 发布包..."

# 清理并创建输出目录
rm -rf release
mkdir -p release

# 1. 创建源码包
echo "创建源码包..."
mkdir -p release/ale-my-eyes-source
cp -r ale-core ale-cli ale-gui ale-server scripts release/ale-my-eyes-source/
cp Cargo.toml README.md LICENSE release/ale-my-eyes-source/

# 创建源码包的配置示例
mkdir -p release/ale-my-eyes-source/config
cat > release/ale-my-eyes-source/config/config.json.example << 'EOF'
{
  "cloud_api": {
    "provider": "openai",
    "api_key": "sk-your-api-key-here",
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
  }
}
EOF

# 创建构建说明
cat > release/ale-my-eyes-source/BUILD.md << 'EOF'
# Ale, My Eyes! 构建指南

## 环境要求
- Rust 1.70.0 或更高版本
- Cargo 包管理器

## 构建步骤

### Linux
```bash
./scripts/package-linux.sh
```

### Windows
```bash
./scripts/package-windows.sh
```

### Android
```bash
export ANDROID_NDK_ROOT=/path/to/ndk
./scripts/package-android.sh
```

## 配置
1. 复制 `config/config.json.example` 为 `config/config.json`
2. 设置您的 OpenAI API 密钥
3. 根据需要调整其他配置

## 运行
```bash
cargo run --bin ale-server
cargo run --bin ale-gui
```
EOF

# 创建压缩包
cd release
tar -czf ale-my-eyes-source.tar.gz ale-my-eyes-source/
cd ..

# 2. 创建快速开始包
echo "创建快速开始包..."
mkdir -p release/ale-my-eyes-quickstart
cp README.md LICENSE release/ale-my-eyes-quickstart/

# 创建快速开始指南
cat > release/ale-my-eyes-quickstart/快速开始.md << 'EOF'
# Ale, My Eyes! 快速开始

## 下载
从 GitHub Releases 下载适合您平台的版本。

## 配置
1. 解压安装包
2. 编辑 `config/config.json` 文件
3. 设置您的 OpenAI API 密钥

## 使用
### Windows
运行 `start-server.bat` 和 `start-gui.bat`

### Linux
运行 `./start.sh`

### Android
安装 APK 文件

## 功能
- 语音识别：麦克风输入
- 语音合成：语音反馈
- 图像描述：屏幕内容理解
- 智能推理：自动选择最佳方式

## 获取帮助
访问 GitHub 项目主页获取更多信息。
EOF

# 创建压缩包
cd release
tar -czf ale-my-eyes-quickstart.tar.gz ale-my-eyes-quickstart/
cd ..

# 3. 创建文档包
echo "创建文档包..."
mkdir -p release/ale-my-eyes-docs
cp README.md LICENSE release/ale-my-eyes-docs/

# 创建API文档
cat > release/ale-my-eyes-docs/API.md << 'EOF'
# Ale, My Eyes! API 文档

## 核心 API

### AleEngine
主引擎，整合所有功能。

#### 方法
- `new(config_path)` - 创建引擎实例
- `transcribe(audio_data)` - 语音识别
- `synthesize(text)` - 语音合成
- `describe_image(image_data)` - 图像描述
- `auto_download_models()` - 自动下载模型

### CloudApi
云端API集成。

#### 支持的提供商
- OpenAI (GPT-4o, Whisper, TTS)
- Anthropic (Claude)
- 自定义API

### ModelManager
模型管理器。

#### 功能
- 自动下载推荐模型
- 根据设备性能选择模型
- 离线/在线模式切换

## 配置 API

### AppConfig
应用配置。

#### 字段
- `cloud_api` - 云端API配置
- `models` - 模型配置
- `inference` - 推理配置
- `audio` - 音频配置
- `ui` - 界面配置

## 错误处理

### AleError
错误类型。

#### 变体
- `AsrError` - 语音识别错误
- `TtsError` - 语音合成错误
- `VlmError` - 视觉理解错误
- `CloudApiError` - 云端API错误
- `ConfigError` - 配置错误
EOF

# 创建压缩包
cd release
tar -czf ale-my-eyes-docs.tar.gz ale-my-eyes-docs/
cd ..

echo "发布包创建完成！"
echo ""
echo "生成的文件："
echo "  - release/ale-my-eyes-source.tar.gz (源码包)"
echo "  - release/ale-my-eyes-quickstart.tar.gz (快速开始包)"
echo "  - release/ale-my-eyes-docs.tar.gz (文档包)"
echo ""
echo "文件大小："
ls -lh release/*.tar.gz
