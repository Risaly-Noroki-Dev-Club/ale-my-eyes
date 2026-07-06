use crate::AppWindow;
use slint::ComponentHandle;

/// Android 入口点 — Slint + android-activity 后端
///
/// 生命周期说明：
/// - `android_main` 在 Activity.onCreate 后被调用
/// - Slint 的 `window.run()` 内部运行 android-activity 事件循环，阻塞直到 Activity 销毁
/// - 前台服务独立于 Activity 生命周期运行，保证后台持续监听
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Err(error) = slint::android::init(app) {
        tracing::error!("Failed to initialize Slint Android backend: {}", error);
        return;
    }

    // 1. 请求运行时权限（CAMERA、RECORD_AUDIO）
    request_runtime_permissions();

    // 2. 启动前台服务（通知栏常驻 + WakeLock）
    start_foreground_service();

    // 3. 创建主窗口
    let window = match AppWindow::new() {
        Ok(window) => window,
        Err(error) => {
            tracing::error!("Failed to create Android app window: {}", error);
            return;
        }
    };

    // 4. 初始化应用逻辑（引擎、VAD、回调等）
    crate::setup_app(&window);

    // 5. 初始化自动化引擎（需要 ndk-context 已就绪）
    init_android_automation();

    // 6. 更新通知栏状态
    update_notification("就绪，正在监听语音指令");

    // 7. 运行事件循环（阻塞直到 Activity 销毁）
    if let Err(error) = window.run() {
        tracing::error!("Android app exited with error: {}", error);
    }

    // 8. Activity 销毁后清理：停止前台服务
    stop_foreground_service();
    tracing::info!("Android app shutdown complete");
}

// ── 权限管理 ──────────────────────────────────────────────────

/// 请求 Android 运行时权限（CAMERA、RECORD_AUDIO、POST_NOTIFICATIONS）
fn request_runtime_permissions() {
    use jni::objects::JValue;

    let ctx = ndk_context::android_context();
    let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(e) => {
            tracing::error!("Failed to get JavaVM for permissions: {}", e);
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            tracing::error!("Failed to attach thread for permissions: {}", e);
            return;
        }
    };

    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    let permissions = [
        "android.permission.RECORD_AUDIO",
        "android.permission.CAMERA",
        "android.permission.POST_NOTIFICATIONS",
    ];

    let mut to_request = Vec::new();

    for perm in &permissions {
        let jperm = match env.new_string(perm) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to create permission string: {}", e);
                continue;
            }
        };

        let result = env.call_method(
            &activity,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&jperm)],
        );

        match result {
            Ok(val) => {
                let granted = val.i().unwrap_or(-1);
                if granted == 0 {
                    tracing::info!("Permission already granted: {}", perm);
                } else {
                    tracing::info!("Permission not granted, queuing: {}", perm);
                    to_request.push(perm);
                }
            }
            Err(e) => {
                tracing::warn!("checkSelfPermission failed for {}: {}", perm, e);
            }
        }
    }

    // 批量请求未授予的权限（一次系统对话框）
    if !to_request.is_empty() {
        let array_len = to_request.len() as i32;
        let jarray = env.new_object_array(
            array_len,
            "java/lang/String",
            jni::objects::JObject::null(),
        );

        if let Ok(ref array) = jarray {
            for (i, perm) in to_request.iter().enumerate() {
                if let Ok(jperm) = env.new_string(perm) {
                    let _ = env.set_object_array_element(array, i as i32, jperm);
                }
            }

            match env.call_method(
                &activity,
                "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[JValue::Object(array), JValue::Int(1001)],
            ) {
                Ok(_) => tracing::info!("Batch permission request sent for {} permissions", array_len),
                Err(e) => tracing::error!("requestPermissions failed: {}", e),
            }
        }
    }
}

// ── 前台服务管理 ──────────────────────────────────────────────

/// 启动前台服务（通知栏常驻 + WakeLock）
fn start_foreground_service() {
    let ctx = ndk_context::android_context();
    let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(e) => {
            tracing::error!("Failed to get JavaVM for foreground service: {}", e);
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            tracing::error!("Failed to attach thread for foreground service: {}", e);
            return;
        }
    };

    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    match env.find_class("com/alemyeyes/foreground/AleForegroundService") {
        Ok(service_class) => {
            match env.call_static_method(
                &service_class,
                "startService",
                "(Landroid/content/Context;)V",
                &[jni::objects::JValue::Object(&activity)],
            ) {
                Ok(_) => tracing::info!("Foreground service start requested"),
                Err(e) => tracing::error!("Failed to start foreground service: {}", e),
            }
        }
        Err(e) => {
            tracing::error!("AleForegroundService class not found: {}", e);
            tracing::info!("Make sure Java classes are compiled and included in the APK");
        }
    }
}

/// 停止前台服务
fn stop_foreground_service() {
    let ctx = ndk_context::android_context();
    let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(vm) => vm,
        Err(e) => {
            tracing::error!("Failed to get JavaVM to stop service: {}", e);
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            tracing::error!("Failed to attach thread to stop service: {}", e);
            return;
        }
    };

    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    match env.find_class("com/alemyeyes/foreground/AleForegroundService") {
        Ok(service_class) => {
            match env.call_static_method(
                &service_class,
                "stopService",
                "(Landroid/content/Context;)V",
                &[jni::objects::JValue::Object(&activity)],
            ) {
                Ok(_) => tracing::info!("Foreground service stop requested"),
                Err(e) => tracing::error!("Failed to stop foreground service: {}", e),
            }
        }
        Err(e) => {
            tracing::warn!("AleForegroundService class not found (stop): {}", e);
        }
    }
}

/// 更新通知栏文字
fn update_notification(text: &str) {
    let vm = match get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            tracing::warn!("Failed to get JavaVM for notification update: {}", e);
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            tracing::warn!("Failed to attach thread for notification update: {}", e);
            return;
        }
    };

    match env.find_class("com/alemyeyes/foreground/AleForegroundService") {
        Ok(service_class) => {
            match env.call_static_method(
                &service_class,
                "getInstance",
                "()Lcom/alemyeyes/foreground/AleForegroundService;",
                &[],
            ) {
                Ok(val) => {
                    if let Ok(instance) = val.l() {
                        if !instance.is_null() {
                            let jtext = env.new_string(text).unwrap();
                            let _ = env.call_method(
                                &instance,
                                "updateNotification",
                                "(Ljava/lang/String;)V",
                                &[jni::objects::JValue::Object(&jtext)],
                            );
                        }
                    }
                }
                Err(e) => tracing::debug!("getInstance failed (service may not be running): {}", e),
            }
        }
        Err(_) => {}
    }
}

// ── 自动化初始化 ──────────────────────────────────────────────

/// 初始化 Android 自动化引擎
fn init_android_automation() {
    match crate::android_automation::AndroidAutomationEngine::new(
        crate::android_automation::AndroidAutomationConfig::default(),
    )
    .init()
    {
        Ok(()) => {
            tracing::info!("Android automation engine initialized successfully");
        }
        Err(e) => {
            tracing::warn!("Android automation engine init failed (non-fatal): {}", e);
        }
    }
}

// ── 辅助函数 ──────────────────────────────────────────────────

fn get_java_vm() -> anyhow::Result<jni::JavaVM> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| anyhow::anyhow!("Failed to get JavaVM: {}", e))?;
    Ok(vm)
}
