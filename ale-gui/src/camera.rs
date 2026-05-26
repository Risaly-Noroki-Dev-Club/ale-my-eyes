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

/// 初始化 Android 相机（通过 JNI）
#[cfg(target_os = "android")]
fn init_camera(
    latest_frame: Arc<Mutex<Option<CameraFrame>>>,
    running: Arc<Mutex<bool>>,
    width: u32,
    height: u32,
) -> Result<()> {
    use jni::objects::{JObject, JValue};
    use ndk_context::android_context;

    let ctx = android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get JVM: {}", e)))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to attach thread: {}", e)))?;

    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    // 获取 CameraManager
    let camera_service = env
        .call_method(
            &activity,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&env.new_string("camera")?.into())],
        )
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get CameraManager: {}", e)))?
        .l()
        .map_err(|e| {
            AleError::Other(anyhow::anyhow!("Failed to get CameraManager object: {}", e))
        })?;

    // 获取后置相机 ID
    let camera_ids = env
        .call_method(
            &camera_service,
            "getCameraIdList",
            "()[Ljava/lang/String;",
            &[],
        )
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get camera IDs: {}", e)))?
        .l()
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get camera IDs array: {}", e)))?;

    let camera_ids_array = env
        .get_array_length(&camera_ids.into())
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get array length: {}", e)))?;

    let mut back_camera_id: Option<String> = None;
    for i in 0..camera_ids_array {
        let id = env
            .get_object_array_element(&camera_ids.into(), i)
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get camera ID: {}", e)))?
            .l()
            .map_err(|e| {
                AleError::Other(anyhow::anyhow!("Failed to get camera ID string: {}", e))
            })?;

        let id_str: String = env
            .get_string(&id.into())
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to convert string: {}", e)))?
            .into();

        // 检查是否为后置相机
        let characteristics = env
            .call_method(
                &camera_service,
                "getCameraCharacteristics",
                "(Ljava/lang/String;)Landroid/hardware/camera2/CameraCharacteristics;",
                &[JValue::Object(&env.new_string(&id_str)?.into())],
            )
            .map_err(|e| {
                AleError::Other(anyhow::anyhow!(
                    "Failed to get camera characteristics: {}",
                    e
                ))
            })?
            .l()
            .map_err(|e| {
                AleError::Other(anyhow::anyhow!(
                    "Failed to get camera characteristics object: {}",
                    e
                ))
            })?;

        let lens_facing = env
            .get_static_field(
                "android/hardware/camera2/CameraCharacteristics",
                "LENS_FACING",
                "Landroid/hardware/camera2/CameraCharacteristics$Key;",
            )
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get LENS_FACING key: {}", e)))?
            .l()
            .map_err(|e| {
                AleError::Other(anyhow::anyhow!(
                    "Failed to get LENS_FACING key object: {}",
                    e
                ))
            })?;

        let facing = env
            .call_method(
                &characteristics,
                "get",
                "(Landroid/hardware/camera2/CameraCharacteristics$Key;)Ljava/lang/Object;",
                &[JValue::Object(&lens_facing)],
            )
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get lens facing: {}", e)))?
            .l()
            .map_err(|e| {
                AleError::Other(anyhow::anyhow!("Failed to get lens facing value: {}", e))
            })?;

        let facing_int = env
            .call_method(&facing, "intValue", "()I", &[])
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get int value: {}", e)))?
            .i()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get int: {}", e)))?;

        // LENS_FACING_BACK = 1
        if facing_int == 1 {
            back_camera_id = Some(id_str);
            break;
        }
    }

    let camera_id =
        back_camera_id.ok_or_else(|| AleError::Other(anyhow::anyhow!("No back camera found")))?;

    tracing::info!("Opening camera: {}", camera_id);

    // 注意：完整的 Camera2 API 集成需要更多代码
    // 这里提供框架，实际实现需要：
    // 1. 创建 CaptureSession
    // 2. 设置 ImageReader
    // 3. 处理回调获取帧数据
    // 4. YUV_420_888 到 RGBA 转换

    // 为了简化，我们使用一个轮询方案
    // 实际产品中应该使用 Camera2 的回调机制

    while {
        let r = running.lock().unwrap();
        *r
    } {
        // TODO: 从 ImageReader 获取帧并转换
        std::thread::sleep(std::time::Duration::from_millis(33)); // ~30fps
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
