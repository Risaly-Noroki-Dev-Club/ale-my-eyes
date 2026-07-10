use super::{ExecutionResult, PlatformCapabilities};
use crate::automation_ios::{AutomationConfig, IosAutomationEngine};
use crate::camera_ios::{CameraConfig, IosCamera};
use ale_core::actions::ActionPlan;
use ale_core::{AleError, Result};

/// iOS 平台服务：AVFoundation 相机 + 有限自动化
pub struct IosPlatform {
    camera: Option<IosCamera>,
    automation: Option<IosAutomationEngine>,
}

impl IosPlatform {
    pub fn new() -> Self {
        let mut platform = Self {
            camera: None,
            automation: None,
        };
        platform.init();
        platform
    }

    fn init(&mut self) {
        // 初始化 iOS 相机
        let mut cam = IosCamera::new(CameraConfig::default());
        if let Err(e) = cam.start() {
            tracing::warn!("iOS camera failed to start: {}", e);
        } else {
            self.camera = Some(cam);
        }

        // 初始化 iOS 自动化引擎（有限支持）
        match IosAutomationEngine::new(AutomationConfig::default()) {
            Ok(ae) => self.automation = Some(ae),
            Err(e) => tracing::warn!("iOS automation engine failed: {}", e),
        }

        tracing::info!("iOS platform services initialized (limited automation support)");
    }
}

impl super::PlatformService for IosPlatform {
    fn capture_image(&self) -> Option<Vec<u8>> {
        self.camera.as_ref()?.latest_frame_jpeg(80)
    }

    fn execute_plan(&self, plan: &ActionPlan, approved: bool) -> Result<ExecutionResult> {
        let auto = self
            .automation
            .as_ref()
            .ok_or_else(|| AleError::Other(anyhow::anyhow!("iOS 自动化引擎不可用")))?;

        let result = auto.execute_plan(plan, approved)?;
        Ok(ExecutionResult {
            actions_executed: result.actions_executed,
        })
    }

    fn is_automation_ready(&self) -> bool {
        self.automation.is_some()
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            image_capture: self.camera.is_some(),
            automation: self.automation.is_some(),
            local_microphone: true,
        }
    }
}
