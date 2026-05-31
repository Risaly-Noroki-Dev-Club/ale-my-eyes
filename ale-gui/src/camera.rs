use ale_core::{AleError, Result};
use std::sync::{Arc, Mutex};

/// 相机帧数据
#[derive(Clone)]
pub struct CameraFrame {
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
}

/// 相机配置
#[derive(Debug, Clone)]
pub struct CameraConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
        }
    }
}

/// Android 相机状态
pub struct AndroidCamera {
    latest_frame: Arc<Mutex<Option<CameraFrame>>>,
    running: Arc<Mutex<bool>>,
    config: CameraConfig,
}

impl AndroidCamera {
    pub fn new(config: CameraConfig) -> Self {
        Self {
            latest_frame: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
            config,
        }
    }

    /// 打开相机并开始预览
    pub fn start(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            return Err(AleError::Other(anyhow::anyhow!(
                "Android camera capture is not implemented yet"
            )));
        }

        #[cfg(not(target_os = "android"))]
        {
            return Err(AleError::Other(anyhow::anyhow!(
                "Camera only available on Android"
            )));
        }

        #[allow(unreachable_code)]
        self.start_impl()
    }

    #[allow(dead_code)]
    fn start_impl(&self) -> Result<()> {
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
        let width = self.config.width;
        let height = self.config.height;

        // 在后台线程中初始化相机
        std::thread::spawn(move || {
            if let Err(e) = init_camera(latest_frame, running, width, height) {
                tracing::error!("Camera initialization failed: {}", e);
            }
        });

        Ok(())
    }

    /// 获取最新帧的 JPEG 数据（用于发送给 API）
    pub fn latest_frame_jpeg(&self, quality: u8) -> Option<Vec<u8>> {
        let frame = self.latest_frame()?;
        frame_to_jpeg(&frame, quality).ok()
    }

    /// 停止相机
    pub fn stop(&self) {
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
    }

    /// 获取最新帧
    pub fn latest_frame(&self) -> Option<CameraFrame> {
        self.latest_frame.lock().ok()?.clone()
    }

    /// 立即捕获一帧
    pub fn capture_frame(&self) -> Result<CameraFrame> {
        self.latest_frame()
            .ok_or_else(|| AleError::Other(anyhow::anyhow!("No camera frame available")))
    }
}

impl Drop for AndroidCamera {
    fn drop(&mut self) {
        self.stop();
    }
}

fn frame_to_jpeg(frame: &CameraFrame, quality: u8) -> Result<Vec<u8>> {
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

/// 初始化 Android 相机（通过 JNI）
#[cfg(target_os = "android")]
fn init_camera(
    _latest_frame: Arc<Mutex<Option<CameraFrame>>>,
    running: Arc<Mutex<bool>>,
    _width: u32,
    _height: u32,
) -> Result<()> {
    while {
        let Ok(r) = running.lock() else {
            tracing::warn!("Camera running flag lock poisoned");
            return Ok(());
        };
        *r
    } {
        // TODO: Wire Camera2/ImageReader callbacks here. Keep the worker alive for now.
        std::thread::sleep(std::time::Duration::from_millis(33));
    }

    Ok(())
}

#[cfg(not(target_os = "android"))]
fn init_camera(
    _latest_frame: Arc<Mutex<Option<CameraFrame>>>,
    _running: Arc<Mutex<bool>>,
    _width: u32,
    _height: u32,
) -> Result<()> {
    Err(AleError::Other(anyhow::anyhow!(
        "Camera only available on Android"
    )))
}

/// YUV_420_888 到 RGBA 转换
pub fn yuv420_to_rgba(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: u32,
    height: u32,
    y_stride: u32,
    uv_stride: u32,
) -> Vec<u8> {
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for row in 0..height {
        for col in 0..width {
            let y_idx = (row * y_stride + col) as usize;
            let uv_row = row / 2;
            let uv_col = col / 2;
            let uv_idx = (uv_row * uv_stride + uv_col) as usize;

            let y = y_plane.get(y_idx).copied().unwrap_or(128) as f32;
            let u = u_plane.get(uv_idx).copied().unwrap_or(128) as f32 - 128.0;
            let v = v_plane.get(uv_idx).copied().unwrap_or(128) as f32 - 128.0;

            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344 * u - 0.714 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

            let rgba_idx = ((row * width + col) * 4) as usize;
            rgba[rgba_idx] = r;
            rgba[rgba_idx + 1] = g;
            rgba[rgba_idx + 2] = b;
            rgba[rgba_idx + 3] = 255;
        }
    }

    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_to_rgba() {
        let width = 2;
        let height = 2;
        let y = vec![128, 128, 128, 128];
        let u = vec![128];
        let v = vec![128];

        let rgba = yuv420_to_rgba(&y, &u, &v, width, height, 2, 1);
        assert_eq!(rgba.len(), 16); // 2*2*4
                                    // 灰色像素 (Y=128, U=128, V=128) -> R≈128, G≈128, B≈128
        assert!((rgba[0] as i32 - 128).abs() < 5);
        assert!((rgba[1] as i32 - 128).abs() < 5);
        assert!((rgba[2] as i32 - 128).abs() < 5);
        assert_eq!(rgba[3], 255); // Alpha
    }
}
