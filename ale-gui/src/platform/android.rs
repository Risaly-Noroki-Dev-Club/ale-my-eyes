use super::ExecutionResult;
use ale_core::actions::ActionPlan;
use ale_core::{AleError, Result};

/// Android 平台服务：仅作为局域网指令入口，不在本机执行自动化。
pub struct AndroidPlatform;

impl AndroidPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl super::PlatformService for AndroidPlatform {
    fn capture_image(&self) -> Option<Vec<u8>> {
        None
    }

    fn execute_plan(&self, _plan: &ActionPlan) -> Result<ExecutionResult> {
        Err(AleError::Other(anyhow::anyhow!(
            "Android 客户端只负责接收指令，请连接桌面端执行"
        )))
    }

    fn is_automation_ready(&self) -> bool {
        false
    }
}
