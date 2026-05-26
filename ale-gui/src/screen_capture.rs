use ale_core::{AleError, Result};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 屏幕帧数据
#[derive(Clone)]
pub struct ScreenFrame {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    pub timestamp: Instant,
}

/// 屏幕捕获配置
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// 截图间隔
    pub interval: Duration,
    /// 缩放比例（0.0-1.0）
    pub scale: f32,
    /// JPEG 质量（用于发送给 API）
    pub jpeg_quality: u8,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3),
            scale: 0.5,
            jpeg_quality: 80,
        }
    }
}

/// 屏幕捕获器（Desktop only）
pub struct ScreenCapture {
    latest_frame: Arc<Mutex<Option<ScreenFrame>>>,
    running: Arc<Mutex<bool>>,
    config: CaptureConfig,
}

impl ScreenCapture {
    pub fn new(config: CaptureConfig) -> Self {
        Self {
            latest_frame: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
            config,
        }
    }

    /// 开始持续捕获
    pub fn start(&self) -> Result<()> {
        let mut running = self
            .running
            .lock()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to lock running flag: {}", e)))?;

        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        let latest_frame = self.latest_frame.clone();
        let running = self.running.clone();
        let interval = self.config.interval;
        let scale = self.config.scale;

        thread::spawn(move || {
            while {
                let r = running.lock().unwrap();
                *r
            } {
                match capture_primary_monitor(scale) {
                    Ok(frame) => {
                        let mut lf = latest_frame.lock().unwrap();
                        *lf = Some(frame);
                    }
                    Err(e) => {
                        tracing::warn!("Screen capture failed: {}", e);
                    }
                }
                thread::sleep(interval);
            }
        });

        Ok(())
    }

    /// 停止捕获
    pub fn stop(&self) {
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }

    /// 获取最新帧
    pub fn latest_frame(&self) -> Option<ScreenFrame> {
        self.latest_frame.lock().ok()?.clone()
    }

    /// 获取最新帧的 JPEG 数据（用于发送给 API）
    pub fn latest_frame_jpeg(&self) -> Option<Vec<u8>> {
        let frame = self.latest_frame()?;
        frame_to_jpeg(&frame, self.config.jpeg_quality).ok()
    }

    /// 立即截取一帧
    pub fn capture_now(&self) -> Result<ScreenFrame> {
        capture_primary_monitor(self.config.scale)
    }
}

/// 捕获主显示器
fn capture_primary_monitor(scale: f32) -> Result<ScreenFrame> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to enumerate monitors: {}", e)))?;

    let monitor = monitors
        .into_iter()
        .next()
        .ok_or_else(|| AleError::Other(anyhow::anyhow!("No monitors found")))?;

    let image = monitor
        .capture_image()
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to capture screen: {}", e)))?;

    let width = image.width();
    let height = image.height();

    // 缩放
    let (scaled_w, scaled_h, rgba_data) = if scale < 1.0 {
        let new_w = (width as f32 * scale) as u32;
        let new_h = (height as f32 * scale) as u32;
        let resized =
            image::imageops::resize(&image, new_w, new_h, image::imageops::FilterType::Nearest);
        (new_w, new_h, resized.into_raw())
    } else {
        (width, height, image.into_raw())
    };

    Ok(ScreenFrame {
        width: scaled_w,
        height: scaled_h,
        rgba_data,
        timestamp: Instant::now(),
    })
}

/// 将帧转换为 JPEG 字节
fn frame_to_jpeg(frame: &ScreenFrame, quality: u8) -> Result<Vec<u8>> {
    let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba_data.clone())
        .ok_or_else(|| AleError::Other(anyhow::anyhow!("Failed to create image from frame")))?;

    let rgb_img = image::DynamicImage::ImageRgba8(img).to_rgb8();

    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    rgb_img
        .write_with_encoder(encoder)
        .map_err(|e| AleError::Other(anyhow::anyhow!("JPEG encode failed: {}", e)))?;

    Ok(buf.into_inner())
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.interval, Duration::from_secs(3));
        assert_eq!(config.scale, 0.5);
        assert_eq!(config.jpeg_quality, 80);
    }
}
