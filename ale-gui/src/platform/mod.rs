use ale_core::actions::ActionPlan;
use ale_core::Result;

/// 统一的自动化执行结果
pub struct ExecutionResult {
    pub actions_executed: usize,
}

/// 平台抽象 trait。
/// Desktop 负责屏幕捕获和执行；Android 目前只作为局域网指令入口骨架。
pub trait PlatformService: Send + Sync {
    /// 捕获当前屏幕画面，返回 JPEG 字节。Android 客户端暂不提供本机画面。
    fn capture_image(&self) -> Option<Vec<u8>>;

    /// 执行自动化操作计划
    fn execute_plan(&self, plan: &ActionPlan) -> Result<ExecutionResult>;

    /// 自动化引擎是否就绪
    fn is_automation_ready(&self) -> bool;
}

/// 为当前编译目标创建平台服务实例
pub fn create_platform() -> Box<dyn PlatformService> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        Box::new(desktop::DesktopPlatform::new())
    }
    #[cfg(target_os = "android")]
    {
        Box::new(android::AndroidPlatform::new())
    }
    #[cfg(target_os = "ios")]
    {
        Box::new(ios::IosPlatform::new())
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "ios")]
mod ios;
