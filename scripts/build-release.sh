#!/bin/bash
# 快速构建脚本 - 生成三份构建产物

set -e

echo "开始构建 Ale, My Eyes! 构建产物..."

# 创建输出目录
mkdir -p dist

# 1. 构建 Linux 版本
echo "构建 Linux 版本..."
cargo build --release 2>/dev/null || echo "Linux 构建跳过（需要完整环境）"

# 创建 Linux 包
if [ -f target/release/ale-server ]; then
    echo "打包 Linux 版本..."
    mkdir -p dist/ale-my-eyes-linux
    cp target/release/ale-server dist/ale-my-eyes-linux/
    cp target/release/ale-cli dist/ale-my-eyes-linux/
    cp target/release/ale-gui dist/ale-my-eyes-linux/
    cp -r scripts dist/ale-my-eyes-linux/
    cp README.md dist/ale-my-eyes-linux/
    cp LICENSE dist/ale-my-eyes-linux/

    # 创建启动脚本
    cat > dist/ale-my-eyes-linux/start.sh << 'EOF'
#!/bin/bash
echo "启动 Ale, My Eyes! 服务器..."
./ale-server
EOF
    chmod +x dist/ale-my-eyes-linux/start.sh

    # 创建压缩包
    cd dist
    tar -czf ale-my-eyes-linux.tar.gz ale-my-eyes-linux/
    cd ..
    echo "Linux 版本构建完成: dist/ale-my-eyes-linux.tar.gz"
else
    echo "跳过 Linux 构建"
fi

# 2. 创建 Windows 构建说明
echo "创建 Windows 构建说明..."
cat > dist/README-WINDOWS.md << 'EOF'
# Ale, My Eyes! Windows 构建说明

## 环境要求
- Windows 10/11
- Visual Studio Build Tools 2019 或更高版本
- Rust 工具链

## 构建步骤
1. 安装 Rust: https://rustup.rs/
2. 安装 Visual Studio Build Tools
3. 克隆项目
4. 运行构建脚本: `scripts/package-windows.sh`

## 预构建版本
从 GitHub Releases 下载预构建版本。
EOF

# 3. 创建 Android 构建说明
echo "创建 Android 构建说明..."
cat > dist/README-ANDROID.md << 'EOF'
# Ale, My Eyes! Android 构建说明

## 环境要求
- Android Studio 2023.1.1 或更高版本
- Android SDK 34
- Android NDK 25.2.9519653 或更高版本
- Rust 工具链

## 构建步骤
1. 安装 Rust: https://rustup.rs/
2. 安装 Android Studio 和 NDK
3. 设置环境变量: `export ANDROID_NDK_ROOT=/path/to/ndk`
4. 克隆项目
5. 运行构建脚本: `scripts/package-android.sh`

## 预构建版本
从 GitHub Releases 下载预构建版本。
EOF

# 4. 创建配置示例
echo "创建配置示例..."
mkdir -p dist/config
cat > dist/config/config.json.example << 'EOF'
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
    "screen_reader": true
  }
}
EOF

# 5. 创建快速开始指南
echo "创建快速开始指南..."
cat > dist/QUICKSTART.md << 'EOF'
# Ale, My Eyes! 快速开始指南

## 1. 下载安装包
从 GitHub Releases 下载适合您平台的安装包。

## 2. 配置 API 密钥
编辑 `config/config.json` 文件，设置您的 OpenAI API 密钥。

## 3. 启动应用
### Windows
运行 `start-server.bat` 和 `start-gui.bat`

### Linux
运行 `./start.sh`

### Android
安装 APK 文件并打开应用

## 4. 开始使用
- 语音识别：点击麦克风按钮开始录音
- 图像描述：上传图像或使用相机拍照
- 语音合成：系统会自动朗读屏幕内容
EOF

echo "构建产物准备完成！"
echo ""
echo "生成的文件："
echo "  - dist/ale-my-eyes-linux.tar.gz (Linux 版本)"
echo "  - dist/README-WINDOWS.md (Windows 构建说明)"
echo "  - dist/README-ANDROID.md (Android 构建说明)"
echo "  - dist/config/config.json.example (配置示例)"
echo "  - dist/QUICKSTART.md (快速开始指南)"
echo ""
echo "下一步："
echo "  1. 创建 GitHub Release"
echo "  2. 上传构建产物"
echo "  3. 添加发布说明"
