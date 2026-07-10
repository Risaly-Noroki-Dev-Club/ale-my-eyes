use super::ExecutionResult;
use crate::automation::{AutomationConfig, AutomationEngine};
use crate::screen_capture::{CaptureConfig, ScreenCapture};
use ale_core::actions::ActionPlan;
use ale_core::{AleError, Result};
use std::sync::Mutex;

/// 桌面平台服务：屏幕捕获 + enigo 自动化
pub struct DesktopPlatform {
    screen_capture: Option<ScreenCapture>,
    automation: Option<Mutex<AutomationEngine>>,
}

impl DesktopPlatform {
    pub fn new() -> Self {
        let mut platform = Self {
            screen_capture: None,
            automation: None,
        };
        platform.init();
        platform
    }

    fn init(&mut self) {
        // 启动屏幕捕获
        let sc = ScreenCapture::new(CaptureConfig::default());
        if let Err(e) = sc.start() {
            tracing::warn!("Screen capture failed to start: {}", e);
        } else {
            self.screen_capture = Some(sc);
        }

        // 创建自动化引擎
        match AutomationEngine::new(AutomationConfig::default()) {
            Ok(ae) => self.automation = Some(Mutex::new(ae)),
            Err(e) => tracing::warn!("Automation engine failed: {}", e),
        }
    }
}

impl super::PlatformService for DesktopPlatform {
    fn capture_image(&self) -> Option<Vec<u8>> {
        self.screen_capture.as_ref()?.latest_frame_jpeg()
    }

    fn execute_plan(&self, plan: &ActionPlan) -> Result<ExecutionResult> {
        let auto = self
            .automation
            .as_ref()
            .ok_or_else(|| AleError::Other(anyhow::anyhow!("自动化引擎不可用")))?;

        let mut guard = auto
            .lock()
            .map_err(|e| AleError::Other(anyhow::anyhow!("自动化引擎锁失败: {}", e)))?;

        let result = guard.execute_plan(plan)?;
        Ok(ExecutionResult {
            actions_executed: result.actions_executed,
        })
    }

    fn is_automation_ready(&self) -> bool {
        self.automation.is_some()
    }
}
