use ale_core::AleEngineFactory;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ale-cli")]
#[command(about = "CLI tool for Ale, My Eyes!")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 语音识别
    Transcribe {
        /// 音频文件路径
        #[arg(short, long)]
        audio: PathBuf,

        /// 输出文本文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// 语音合成
    Synthesize {
        /// 要合成的文本
        #[arg(short, long)]
        text: String,

        /// 输出音频文件路径
        #[arg(short, long)]
        output: PathBuf,

        /// 语音，目前云端默认使用 alloy
        #[arg(long)]
        voice: Option<String>,
    },

    /// 图像描述
    Describe {
        /// 图像文件路径
        #[arg(short, long)]
        image: PathBuf,

        /// 输出文本文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// 测试云端连接
    TestConnection,

    /// 显示状态
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Transcribe { audio, output } => {
            let engine = AleEngineFactory::create_default().await?;
            let audio_data = tokio::fs::read(&audio).await?;
            let text = engine.transcribe(&audio_data).await?;
            write_or_print(output, &text).await?;
        }
        Commands::Synthesize {
            text,
            output,
            voice: _,
        } => {
            let engine = AleEngineFactory::create_default().await?;
            let audio = engine.synthesize(&text).await?;
            tokio::fs::write(&output, audio).await?;
            println!("Audio written to {}", output.display());
        }
        Commands::Describe { image, output } => {
            let engine = AleEngineFactory::create_default().await?;
            let image_data = tokio::fs::read(&image).await?;
            let description = engine.describe_image(&image_data).await?;
            write_or_print(output, &description).await?;
        }
        Commands::TestConnection => {
            let engine = AleEngineFactory::create_default().await?;
            match engine.test_cloud_api().await {
                Ok(true) => {
                    println!("云端连接测试成功");
                    let config = engine.config();
                    println!("API URL: {}", config.cloud_api.api_url);
                    println!("Model: {}", config.cloud_api.model);
                }
                Ok(false) => {
                    println!("云端连接测试失败");
                    std::process::exit(1);
                }
                Err(error) => {
                    println!("云端连接测试失败: {error}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Status => {
            let engine = AleEngineFactory::create_default().await?;
            let status = engine.status().await;
            let config = engine.config();
            println!("Ale, My Eyes! CLI");
            println!("Version: 0.1.0");
            println!("Cloud ready: {}", status.cloud_ready);
            println!("TTS ready: {}", status.tts_ready);
            println!("API URL: {}", config.cloud_api.api_url);
            println!("Model: {}", config.cloud_api.model);
            println!("Language: {}", config.ui.language);
        }
    }

    Ok(())
}

async fn write_or_print(output: Option<PathBuf>, text: &str) -> anyhow::Result<()> {
    if let Some(output) = output {
        tokio::fs::write(&output, text).await?;
        println!("Text written to {}", output.display());
    } else {
        println!("{text}");
    }

    Ok(())
}
