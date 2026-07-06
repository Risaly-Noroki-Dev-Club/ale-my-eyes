use ale_core::actions::{Action, ActionPlan, FileOp, MouseButton};
use ale_core::{AleError, Result};
use jni::objects::{JObject, JValue};
use jni::JavaVM;

/// Android 自动化引擎配置
#[derive(Debug, Clone)]
pub struct AndroidAutomationConfig {
    pub require_confirmation: bool,
    pub action_delay_ms: u64,
}

impl Default for AndroidAutomationConfig {
    fn default() -> Self {
        Self {
            require_confirmation: true,
            action_delay_ms: 100,
        }
    }
}

/// Android 自动化执行结果
#[derive(Debug, Clone)]
pub struct AndroidExecutionResult {
    pub success: bool,
    pub actions_executed: usize,
    pub error: Option<String>,
}

/// 通过 ndk-context 获取全局 JavaVM
fn get_java_vm() -> Result<JavaVM> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to get JavaVM: {}", e)))?;
    Ok(vm)
}

/// 获取 AutomationBridge 的全局引用
fn get_bridge_ref(
    env: &mut jni::JNIEnv,
) -> Result<jni::objects::GlobalRef> {
    let bridge_class = env
        .find_class("com/alemyeyes/automation/AutomationBridge")
        .map_err(|e| AleError::Other(anyhow::anyhow!("AutomationBridge class not found: {}", e)))?;

    let bridge_obj = env
        .call_static_method(
            &bridge_class,
            "getInstance",
            "()Lcom/alemyeyes/automation/AutomationBridge;",
            &[],
        )
        .map_err(|e| AleError::Other(anyhow::anyhow!("getInstance failed: {}", e)))?
        .l()
        .map_err(|e| AleError::Other(anyhow::anyhow!("unwrap JObject failed: {}", e)))?;

    let global_ref = env.new_global_ref(bridge_obj).map_err(|e| {
        AleError::Other(anyhow::anyhow!("Failed to create global reference: {}", e))
    })?;

    Ok(global_ref)
}

/// Android 自动化引擎 - 通过 JNI 调用 AccessibilityService
pub struct AndroidAutomationEngine {
    config: AndroidAutomationConfig,
    initialized: bool,
}

impl AndroidAutomationEngine {
    pub fn new(config: AndroidAutomationConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    /// 初始化 JNI 桥接，检查 Java 侧的 AutomationBridge 是否可用
    pub fn init(&mut self) -> Result<()> {
        let vm = get_java_vm()?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to attach thread: {}", e)))?;

        // 验证 AutomationBridge 类存在
        let _ = get_bridge_ref(&mut env)?;
        self.initialized = true;
        tracing::info!("Android automation JNI bridge initialized");
        Ok(())
    }

    /// 检查无障碍服务是否已启用
    pub fn is_accessibility_enabled(&self) -> Result<bool> {
        let vm = get_java_vm()?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to attach thread: {}", e)))?;

        let bridge_ref = get_bridge_ref(&mut env)?;

        let result = env
            .call_method(
                bridge_ref.as_obj(),
                "isAccessibilityServiceEnabled",
                "()Z",
                &[],
            )
            .map_err(|e| AleError::Other(anyhow::anyhow!("isAccessibilityServiceEnabled failed: {}", e)))?
            .z()
            .map_err(|e| AleError::Other(anyhow::anyhow!("unwrap bool failed: {}", e)))?;

        Ok(result)
    }

    /// 跳转到无障碍设置页面
    pub fn open_accessibility_settings(&self) -> Result<()> {
        let vm = get_java_vm()?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to attach thread: {}", e)))?;

        let bridge_ref = get_bridge_ref(&mut env)?;

        env.call_method(
            bridge_ref.as_obj(),
            "openAccessibilitySettings",
            "()V",
            &[],
        )
        .map_err(|e| AleError::Other(anyhow::anyhow!("openAccessibilitySettings failed: {}", e)))?;

        Ok(())
    }

    /// 执行操作计划（不需要外部 JNI 环境）
    pub fn execute_plan(&self, plan: &ActionPlan) -> Result<AndroidExecutionResult> {
        if !self.initialized {
            return Err(AleError::NotInitialized("Android automation engine"));
        }

        if plan.requires_confirmation && self.config.require_confirmation {
            return Err(AleError::Other(anyhow::anyhow!(
                "操作需要用户确认: {}",
                plan.explanation
            )));
        }

        let vm = get_java_vm()?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| AleError::Other(anyhow::anyhow!("Failed to attach thread: {}", e)))?;

        let bridge_ref = get_bridge_ref(&mut env)?;

        let mut executed = 0;
        for action in &plan.actions {
            execute_action(&mut env, &bridge_ref, action)?;
            executed += 1;

            if self.config.action_delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(self.config.action_delay_ms));
            }
        }

        Ok(AndroidExecutionResult {
            success: true,
            actions_executed: executed,
            error: None,
        })
    }
}

/// 执行单个操作
fn execute_action(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    action: &Action,
) -> Result<()> {
    match action {
        Action::Click { x, y, button } => {
            perform_click(env, bridge_ref, *x, *y, *button)?;
        }
        Action::DoubleClick { x, y } => {
            perform_click(env, bridge_ref, *x, *y, MouseButton::Left)?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            perform_click(env, bridge_ref, *x, *y, MouseButton::Left)?;
        }
        Action::MouseMove { x, y } => {
            tracing::debug!("Ignoring mouse move on Android: ({}, {})", x, y);
        }
        Action::Scroll {
            x, y, delta_x, delta_y,
        } => {
            perform_scroll(env, bridge_ref, *x, *y, *delta_x, *delta_y)?;
        }
        Action::Type { text } => {
            perform_type(env, bridge_ref, text)?;
        }
        Action::Key { key, modifiers } => {
            perform_key(env, bridge_ref, key, modifiers)?;
        }
        Action::Wait { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
        }
        Action::OpenApp { name } => {
            perform_open_app(env, bridge_ref, name)?;
        }
        Action::CloseApp { name } => {
            perform_close_app(env, bridge_ref, name)?;
        }
        Action::OpenUrl { url } => {
            perform_open_url(env, bridge_ref, url)?;
        }
        Action::FileOperation {
            operation,
            path,
            target,
        } => {
            perform_file_op(env, bridge_ref, *operation, path, target.as_deref())?;
        }
    }
    Ok(())
}

fn perform_click(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    x: f64,
    y: f64,
    button: MouseButton,
) -> Result<()> {
    let action_type = match button {
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 1,
    };

    env.call_method(
        bridge_ref.as_obj(),
        "performClick",
        "(DDI)V",
        &[
            JValue::Double(x),
            JValue::Double(y),
            JValue::Int(action_type),
        ],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("performClick failed: {}", e)))?;

    Ok(())
}

fn perform_scroll(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    x: f64,
    y: f64,
    delta_x: f64,
    delta_y: f64,
) -> Result<()> {
    env.call_method(
        bridge_ref.as_obj(),
        "performScroll",
        "(DDDD)V",
        &[
            JValue::Double(x),
            JValue::Double(y),
            JValue::Double(delta_x),
            JValue::Double(delta_y),
        ],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("performScroll failed: {}", e)))?;

    Ok(())
}

fn perform_type(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    text: &str,
) -> Result<()> {
    let jtext = env
        .new_string(text)
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    env.call_method(
        bridge_ref.as_obj(),
        "performTypeText",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&jtext)],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("performTypeText failed: {}", e)))?;

    Ok(())
}

fn perform_key(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    key: &str,
    modifiers: &[String],
) -> Result<()> {
    let key_code = map_key_to_android_keycode(key);

    let jmodifiers = env
        .new_object_array(
            modifiers.len() as i32,
            "java/lang/String",
            JObject::null(),
        )
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_object_array failed: {}", e)))?;

    for (i, modifier) in modifiers.iter().enumerate() {
        let jmodifier = env
            .new_string(modifier)
            .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;
        env.set_object_array_element(&jmodifiers, i as i32, jmodifier)
            .map_err(|e| AleError::Other(anyhow::anyhow!("set_object_array_element failed: {}", e)))?;
    }

    env.call_method(
        bridge_ref.as_obj(),
        "performKeyPress",
        "(I[Ljava/lang/String;)V",
        &[JValue::Int(key_code), JValue::Object(&jmodifiers)],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("performKeyPress failed: {}", e)))?;

    Ok(())
}

fn perform_open_app(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    name: &str,
) -> Result<()> {
    let jname = env
        .new_string(name)
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    env.call_method(
        bridge_ref.as_obj(),
        "openApp",
        "(Ljava/lang/String;)Z",
        &[JValue::Object(&jname)],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("openApp failed: {}", e)))?;

    Ok(())
}

fn perform_close_app(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    name: &str,
) -> Result<()> {
    let jname = env
        .new_string(name)
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    env.call_method(
        bridge_ref.as_obj(),
        "closeApp",
        "(Ljava/lang/String;)Z",
        &[JValue::Object(&jname)],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("closeApp failed: {}", e)))?;

    Ok(())
}

fn perform_open_url(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    url: &str,
) -> Result<()> {
    let jurl = env
        .new_string(url)
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    env.call_method(
        bridge_ref.as_obj(),
        "openUrl",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&jurl)],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("openUrl failed: {}", e)))?;

    Ok(())
}

fn perform_file_op(
    env: &mut jni::JNIEnv,
    bridge_ref: &jni::objects::GlobalRef,
    op: FileOp,
    path: &str,
    target: Option<&str>,
) -> Result<()> {
    let op_code = match op {
        FileOp::Create => 0,
        FileOp::Delete => 1,
        FileOp::Move => 2,
        FileOp::Copy => 3,
        FileOp::Rename => 4,
    };

    let jpath = env
        .new_string(path)
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    let jtarget = env
        .new_string(target.unwrap_or(""))
        .map_err(|e| AleError::Other(anyhow::anyhow!("new_string failed: {}", e)))?;

    env.call_method(
        bridge_ref.as_obj(),
        "performFileOperation",
        "(ILjava/lang/String;Ljava/lang/String;)Z",
        &[
            JValue::Int(op_code),
            JValue::Object(&jpath),
            JValue::Object(&jtarget),
        ],
    )
    .map_err(|e| AleError::Other(anyhow::anyhow!("performFileOperation failed: {}", e)))?;

    Ok(())
}

/// 将按键名称映射到 Android KeyEvent KeyCode
pub fn map_key_to_android_keycode(key: &str) -> i32 {
    match key.to_lowercase().as_str() {
        "enter" | "return" => 66,
        "tab" => 61,
        "space" => 62,
        "backspace" => 67,
        "delete" | "del" => 112,
        "escape" | "esc" => 111,
        "up" => 19,
        "down" => 20,
        "left" => 21,
        "right" => 22,
        "home" => 122,
        "end" => 123,
        "pageup" => 92,
        "pagedown" => 93,
        "f1" => 131,
        "f2" => 132,
        "f3" => 133,
        "f4" => 134,
        "f5" => 135,
        "f6" => 136,
        "f7" => 137,
        "f8" => 138,
        "f9" => 139,
        "f10" => 140,
        "f11" => 141,
        "f12" => 142,
        "ctrl" | "control" => 113,
        "alt" => 57,
        "shift" => 59,
        "super" | "win" | "meta" | "cmd" | "command" => 117,
        _ => {
            if key.len() == 1 {
                let ch = key.chars().next().unwrap_or('?');
                match ch {
                    'a'..='z' => (ch as i32) - ('a' as i32) + 29,
                    '0'..='9' => (ch as i32) - ('0' as i32) + 7,
                    _ => 0,
                }
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_key_to_android_keycode() {
        assert_eq!(map_key_to_android_keycode("enter"), 66);
        assert_eq!(map_key_to_android_keycode("a"), 29);
        assert_eq!(map_key_to_android_keycode("0"), 7);
        assert_eq!(map_key_to_android_keycode("ctrl"), 113);
        assert_eq!(map_key_to_android_keycode("unknown"), 0);
    }

    #[test]
    fn test_automation_config_default() {
        let config = AndroidAutomationConfig::default();
        assert!(config.require_confirmation);
        assert_eq!(config.action_delay_ms, 100);
    }
}
