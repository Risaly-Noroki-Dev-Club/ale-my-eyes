use crate::AppWindow;
use slint::ComponentHandle;

/// Android 入口点 — Slint + android-activity 后端。
///
/// Android 客户端现在只作为局域网指令入口，不启动本机自动化、相机或前台服务。
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Err(error) = slint::android::init(app) {
        tracing::error!("Failed to initialize Slint Android backend: {}", error);
        return;
    }

    // 1. 请求运行时权限（RECORD_AUDIO）
    request_runtime_permissions();

    // 2. 创建主窗口
    let window = match AppWindow::new() {
        Ok(window) => window,
        Err(error) => {
            tracing::error!("Failed to create Android app window: {}", error);
            return;
        }
    };

    // 3. 初始化应用逻辑（引擎、VAD、回调等）
    crate::setup_app(&window);

    // 4. 运行事件循环（阻塞直到 Activity 销毁）
    if let Err(error) = window.run() {
        tracing::error!("Android app exited with error: {}", error);
    }

    tracing::info!("Android app shutdown complete");
}

// ── 权限管理 ──────────────────────────────────────────────────

/// 请求 Android 运行时权限（RECORD_AUDIO）
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

    let permissions = ["android.permission.RECORD_AUDIO"];

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
        let jarray =
            env.new_object_array(array_len, "java/lang/String", jni::objects::JObject::null());

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
                Ok(_) => tracing::info!(
                    "Batch permission request sent for {} permissions",
                    array_len
                ),
                Err(e) => tracing::error!("requestPermissions failed: {}", e),
            }
        }
    }
}
