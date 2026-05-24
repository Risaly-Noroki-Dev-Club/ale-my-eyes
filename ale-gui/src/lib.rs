pub mod audio;
pub mod file_picker;
pub mod tts_player;

use ale_core::config::AppConfig;
use ale_core::{AleEngine, AleEngineFactory};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

slint::include_modules!();

pub struct AppState {
    engine: Option<Arc<Mutex<AleEngine>>>,
    recorder: Option<audio::Recorder>,
    recording_started: Option<Instant>,
    auto_speak: bool,
    diagnostics_errors: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: None,
            recorder: None,
            recording_started: None,
            auto_speak: true,
            diagnostics_errors: Vec::new(),
        }
    }
}

pub fn setup_app(app: &AppWindow) {
    let state = Arc::new(Mutex::new(AppState::new()));
    let app_weak = app.as_weak();

    // Initialize engine
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        slint::spawn_local(async move {
            let result = create_engine().await;
            let mut st = state.lock().await;
            let app = app_weak.unwrap();

            match result {
                Ok((engine, config)) => {
                    st.engine = Some(engine);
                    apply_config_to_app(&app, &config);
                    app.set_engine_ready(true);
                    app.set_engine_status("已初始化".into());
                    app.set_status_text("就绪".into());
                    app.set_status_type("ready".into());
                    app.set_api_url(config.cloud_api.api_url.into());
                    app.set_model(config.cloud_api.model.into());
                }
                Err(error) => {
                    let msg = slint::format!("初始化失败: {}", error);
                    app.set_status_text(msg);
                    app.set_status_type("error".into());
                    app.set_engine_status("初始化失败".into());
                    st.diagnostics_errors.push(error.clone());
                    update_errors_display(&app, &st.diagnostics_errors);
                }
            }
        })
        .unwrap();
    }

    // Toggle recording
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_toggle_recording(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let mut st = state.lock().await;
                let app = app_weak.unwrap();

                // Start recording
                if st.recorder.is_none() {
                    match audio::Recorder::start() {
                        Ok(recorder) => {
                            st.recorder = Some(recorder);
                            st.recording_started = Some(Instant::now());
                            app.set_is_recording(true);
                            app.set_status_text("正在录音...".into());
                            app.set_status_type("recording".into());
                        }
                        Err(error) => {
                            app.set_status_text(slint::format!("{}", error));
                            app.set_status_type("error".into());
                            st.diagnostics_errors.push(error);
                            update_errors_display(&app, &st.diagnostics_errors);
                        }
                    }
                    return;
                }

                // Stop recording
                let engine = st.engine.clone();
                let recorder = st.recorder.take();
                let auto_speak = st.auto_speak;
                st.recording_started = None;
                app.set_is_recording(false);

                let Some(engine) = engine else {
                    app.set_status_text("引擎尚未初始化".into());
                    app.set_status_type("error".into());
                    return;
                };

                let Some(recorder) = recorder else {
                    app.set_status_text("录音状态丢失".into());
                    app.set_status_type("error".into());
                    return;
                };

                let audio = match recorder.into_wav_bytes() {
                    Ok(audio) => audio,
                    Err(error) => {
                        app.set_status_text(slint::format!("录音保存失败: {}", error));
                        app.set_status_type("error".into());
                        return;
                    }
                };

                app.set_status_text("正在转写语音...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                // Drop the lock before async work
                drop(st);

                let result = transcribe_audio(engine.clone(), audio).await;
                let mut st = state.lock().await;
                let app = app_weak.unwrap();

                match result {
                    Ok(text) => {
                        app.set_status_text("就绪".into());
                        app.set_status_type("ready".into());
                        app.set_result_title("语音识别结果".into());
                        app.set_result_text(text.clone().into());
                        app.set_result_metadata("".into());
                        app.set_has_result(true);

                        if auto_speak {
                            let speak_str = slint::format!("识别完成: {}", text);
                            drop(st);
                            let app_weak2 = app_weak.clone();
                            slint::spawn_local(async move {
                                let _ = speak_and_play(engine, speak_str.as_str()).await;
                                let app = app_weak2.unwrap();
                                app.set_status_text("就绪".into());
                                app.set_status_type("ready".into());
                            })
                            .unwrap();
                        }
                    }
                    Err(error) => {
                        app.set_status_text(slint::format!("语音识别失败: {}", error));
                        app.set_status_type("error".into());
                        st.diagnostics_errors.push(error);
                        update_errors_display(&app, &st.diagnostics_errors);
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Describe image
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_describe_image(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let engine = st.engine.clone();
                let auto_speak = st.auto_speak;
                drop(st);

                let Some(engine) = engine else {
                    let app = app_weak.unwrap();
                    app.set_status_text("引擎尚未初始化".into());
                    app.set_status_type("error".into());
                    return;
                };

                let app = app_weak.unwrap();
                app.set_status_text("正在选择并描述图片...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                let result = describe_image_inner().await;

                let image_result = match result {
                    Ok((bytes, file_name)) => {
                        let metadata = format!("图片: {}，大小: {} bytes", file_name, bytes.len());
                        let engine_guard = engine.lock().await;
                        match ensure_api_key(engine_guard.config()) {
                            Ok(()) => {
                                let text = engine_guard
                                    .describe_image(&bytes)
                                    .await
                                    .map_err(|e| e.to_string());
                                Some((text, metadata))
                            }
                            Err(e) => Some((Err(e), String::new())),
                        }
                    }
                    Err(e) => Some((Err(e), String::new())),
                };

                let app = app_weak.unwrap();
                let mut st = state.lock().await;

                if let Some((text_result, metadata)) = image_result {
                    match text_result {
                        Ok(text) => {
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());
                            app.set_result_title("图像描述结果".into());
                            app.set_result_text(text.clone().into());
                            app.set_result_metadata(slint::format!("{}", metadata));
                            app.set_has_result(true);
                            app.set_last_image_info(slint::format!("{}", metadata));

                            if auto_speak {
                                let speak_str = slint::format!("图片描述完成: {}", text);
                                drop(st);
                                let app_weak2 = app_weak.clone();
                                slint::spawn_local(async move {
                                    let _ = speak_and_play(engine, speak_str.as_str()).await;
                                    let app = app_weak2.unwrap();
                                    app.set_status_text("就绪".into());
                                    app.set_status_type("ready".into());
                                })
                                .unwrap();
                            }
                        }
                        Err(error) => {
                            app.set_status_text(slint::format!("图片描述失败: {}", error));
                            app.set_status_type("error".into());
                            st.diagnostics_errors.push(error);
                            update_errors_display(&app, &st.diagnostics_errors);
                        }
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Speak result
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_speak_result(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let engine = st.engine.clone();
                drop(st);

                let Some(engine) = engine else {
                    let app = app_weak.unwrap();
                    app.set_status_text("引擎尚未初始化".into());
                    app.set_status_type("error".into());
                    return;
                };

                let app = app_weak.unwrap();
                let result_text: String = app.get_result_text().into();
                if result_text.is_empty() {
                    app.set_status_text("没有可朗读的结果".into());
                    app.set_status_type("error".into());
                    return;
                }

                app.set_status_text("正在朗读结果...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                let result = speak_and_play(engine, &result_text).await;
                let app = app_weak.unwrap();

                match result {
                    Ok(()) => {
                        app.set_status_text("就绪".into());
                        app.set_status_type("ready".into());
                    }
                    Err(error) => {
                        app.set_status_text(slint::format!("朗读失败: {}", error));
                        app.set_status_type("error".into());
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Clear result
    {
        let app_weak = app_weak.clone();
        app.on_clear_result(move || {
            let app = app_weak.unwrap();
            app.set_has_result(false);
            app.set_result_text("".into());
            app.set_result_title("".into());
            app.set_result_metadata("".into());
        });
    }

    // Clear error
    {
        let app_weak = app_weak.clone();
        app.on_clear_error(move || {
            let app = app_weak.unwrap();
            if app.get_status_type().as_str() == "error" {
                app.set_status_text("就绪".into());
                app.set_status_type("ready".into());
            }
        });
    }

    // Save settings
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_save_settings(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let engine = st.engine.clone();
                drop(st);

                let Some(engine) = engine else {
                    let app = app_weak.unwrap();
                    app.set_status_text("引擎尚未初始化".into());
                    app.set_status_type("error".into());
                    return;
                };

                let app = app_weak.unwrap();
                let config = config_from_app(&app);

                app.set_status_text("正在保存设置...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                let result = save_settings(engine, config).await;
                let mut st = state.lock().await;
                let app = app_weak.unwrap();

                match result {
                    Ok((new_engine, new_config)) => {
                        st.engine = Some(new_engine);
                        apply_config_to_app(&app, &new_config);
                        app.set_status_text("就绪".into());
                        app.set_status_type("ready".into());
                        app.set_current_screen(0);
                    }
                    Err(error) => {
                        app.set_status_text(slint::format!("保存失败: {}", error));
                        app.set_status_type("error".into());
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Test connection
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_test_connection(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let engine = st.engine.clone();
                drop(st);

                let Some(engine) = engine else {
                    let app = app_weak.unwrap();
                    app.set_status_text("引擎尚未初始化".into());
                    app.set_status_type("error".into());
                    return;
                };

                let app = app_weak.unwrap();
                app.set_status_text("正在测试云端连接...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                let result = test_connection(engine).await;
                let app = app_weak.unwrap();

                match result {
                    Ok(true) => {
                        app.set_status_text("就绪".into());
                        app.set_status_type("ready".into());
                        app.set_result_title("连接测试结果".into());
                        app.set_result_text("云端连接测试成功".into());
                        app.set_has_result(true);
                    }
                    Ok(false) => {
                        app.set_status_text("云端连接测试失败".into());
                        app.set_status_type("error".into());
                    }
                    Err(error) => {
                        app.set_status_text(slint::format!("连接测试失败: {}", error));
                        app.set_status_type("error".into());
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Settings field callbacks
    {
        let app_weak = app_weak.clone();
        app.on_provider_changed(move |text| {
            app_weak.unwrap().set_provider(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_api_key_changed(move |text| {
            app_weak.unwrap().set_api_key(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_api_url_changed(move |text| {
            app_weak.unwrap().set_api_url(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_model_changed(move |text| {
            app_weak.unwrap().set_model(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_language_changed(move |text| {
            app_weak.unwrap().set_language(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_font_size_changed(move |text| {
            app_weak.unwrap().set_font_size_str(text);
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_high_contrast_changed(move |value| {
            app_weak.unwrap().set_high_contrast(value);
        });
    }
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_auto_speak_changed(move |value| {
            app_weak.unwrap().set_auto_speak(value);
            let state = state.clone();
            slint::spawn_local(async move {
                state.lock().await.auto_speak = value;
            })
            .unwrap();
        });
    }
    {
        let app_weak = app_weak.clone();
        app.on_toggle_api_key_visible(move || {
            let app = app_weak.unwrap();
            app.set_show_api_key(!app.get_show_api_key());
        });
    }

    // Recording timer
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || {
                let state = state.clone();
                let app_weak = app_weak.clone();
                slint::spawn_local(async move {
                    let st = state.lock().await;
                    if let Some(started) = st.recording_started {
                        let elapsed = started.elapsed().as_secs();
                        let app = app_weak.unwrap();
                        app.set_recording_seconds(elapsed as i32);

                        if elapsed >= 60 && st.recorder.is_some() {
                            drop(st);
                            app.invoke_toggle_recording();
                        }
                    }
                })
                .unwrap();
            },
        );
    }
}

fn apply_config_to_app(app: &AppWindow, config: &AppConfig) {
    app.set_provider(config.cloud_api.provider.clone().into());
    app.set_api_key(config.cloud_api.api_key.clone().into());
    app.set_api_url(config.cloud_api.api_url.clone().into());
    app.set_model(config.cloud_api.model.clone().into());
    app.set_language(config.ui.language.clone().into());
    app.set_font_size_str(config.ui.font_size.to_string().into());
    app.set_high_contrast(config.ui.high_contrast);
}

fn config_from_app(app: &AppWindow) -> AppConfig {
    let mut config = AppConfig::default();
    config.cloud_api.provider = app.get_provider().to_string();
    config.cloud_api.api_key = app.get_api_key().to_string();
    config.cloud_api.api_url = app
        .get_api_url()
        .to_string()
        .trim_end_matches('/')
        .to_string();
    config.cloud_api.model = app.get_model().to_string();
    config.ui.language = app.get_language().to_string();
    if let Ok(size) = app.get_font_size_str().to_string().parse::<u32>() {
        config.ui.font_size = size;
    }
    config.ui.high_contrast = app.get_high_contrast();
    config
}

fn update_errors_display(app: &AppWindow, errors: &[String]) {
    let text = if errors.is_empty() {
        String::new()
    } else {
        errors.join("\n")
    };
    app.set_recent_errors(text.into());
}

async fn create_engine() -> Result<(Arc<Mutex<AleEngine>>, AppConfig), String> {
    let engine = AleEngineFactory::create_default()
        .await
        .map_err(|error| error.to_string())?;
    let config = engine.config().clone();
    Ok((Arc::new(Mutex::new(engine)), config))
}

async fn save_settings(
    engine: Arc<Mutex<AleEngine>>,
    config: AppConfig,
) -> Result<(Arc<Mutex<AleEngine>>, AppConfig), String> {
    {
        let mut engine = engine.lock().await;
        engine
            .update_config(config)
            .map_err(|error| error.to_string())?;
    }
    create_engine().await
}

async fn describe_image_inner() -> Result<(Vec<u8>, String), String> {
    file_picker::pick_image().await
}

async fn transcribe_audio(engine: Arc<Mutex<AleEngine>>, audio: Vec<u8>) -> Result<String, String> {
    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    engine
        .transcribe(&audio)
        .await
        .map_err(|error| error.to_string())
}

async fn test_connection(engine: Arc<Mutex<AleEngine>>) -> Result<bool, String> {
    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    engine
        .test_cloud_api()
        .await
        .map_err(|error| error.to_string())
}

async fn speak_and_play(engine: Arc<Mutex<AleEngine>>, text: &str) -> Result<(), String> {
    let audio = {
        let engine = engine.lock().await;
        ensure_api_key(engine.config())?;
        engine
            .synthesize(text)
            .await
            .map_err(|error| error.to_string())?
    };

    tokio::task::spawn_blocking(move || tts_player::play_audio(&audio))
        .await
        .map_err(|error| format!("音频播放任务失败: {error}"))?
}

fn ensure_api_key(config: &AppConfig) -> Result<(), String> {
    if config.cloud_api.api_key.trim().is_empty() {
        return Err("API key 未配置，请先打开设置填写".to_string());
    }
    Ok(())
}
