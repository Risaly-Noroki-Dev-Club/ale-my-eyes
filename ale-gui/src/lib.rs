pub mod audio;
mod conversation;
pub mod file_picker;
pub mod tts_player;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub mod camera;

#[cfg(not(target_os = "android"))]
pub mod screen_capture;

#[cfg(not(target_os = "android"))]
pub mod automation;

use ale_core::actions::ActionPlan;
use ale_core::config::AppConfig;
use ale_core::vad::{VadState, VoiceActivityDetector};
use ale_core::{AleEngine, AleEngineFactory};
use conversation::handle_question_response;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

slint::include_modules!();

pub struct AppState {
    engine: Option<Arc<Mutex<AleEngine>>>,
    recorder: Option<audio::Recorder>,
    recording_started: Option<Instant>,
    auto_speak: bool,
    vad: VoiceActivityDetector,
    vad_active: bool,
    #[cfg(not(target_os = "android"))]
    screen_capture: Option<screen_capture::ScreenCapture>,
    #[cfg(not(target_os = "android"))]
    automation: Option<automation::AutomationEngine>,
    #[cfg(target_os = "android")]
    camera: Option<camera::AndroidCamera>,
    pending_plan: Option<ActionPlan>,
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
            vad: VoiceActivityDetector::with_default_config(),
            vad_active: false,
            #[cfg(not(target_os = "android"))]
            screen_capture: None,
            #[cfg(not(target_os = "android"))]
            automation: None,
            #[cfg(target_os = "android")]
            camera: None,
            pending_plan: None,
        }
    }
}

pub fn setup_app(app: &AppWindow) {
    let state = Arc::new(Mutex::new(AppState::new()));
    let app_weak = app.as_weak();

    // Initialize engine + start monitoring
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        slint::spawn_local(async move {
            let result = create_engine().await;
            let mut st = state.lock().await;
            let app = app_weak.unwrap();

            match result {
                Ok((engine, config)) => {
                    apply_config_to_app(&app, &config);
                    let config_path = ale_core::config::ConfigFactory::create_default()
                        .config_path()
                        .to_string_lossy()
                        .to_string();
                    app.set_config_path(config_path.into());

                    st.engine = Some(engine);
                    app.set_engine_ready(true);
                    app.set_status_text("就绪".into());
                    app.set_status_type("ready".into());

                    initialize_platform_services(&mut st);

                    // Auto-start continuous listening
                    start_continuous_listening(&mut st, &app);
                }
                Err(error) => {
                    app.set_status_text(slint::format!("初始化失败: {}", error));
                    app.set_status_type("error".into());
                }
            }
        })
        .unwrap();
    }

    // VAD timer — checks for speech end every 100ms
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        let vad_timer = slint::Timer::default();
        vad_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                let state = state.clone();
                let app_weak = app_weak.clone();
                slint::spawn_local(async move {
                    let mut st = state.lock().await;
                    if !st.vad_active || st.recorder.is_none() {
                        return;
                    }

                    let samples = if let Some(ref recorder) = st.recorder {
                        recorder.take_samples()
                    } else {
                        return;
                    };

                    if samples.is_empty() {
                        return;
                    }

                    let pcm = ale_core::vad::pcm16_bytes_to_f32(&samples);
                    let mut speech_ended = false;
                    for chunk in pcm.chunks(st.vad.config.frame_size) {
                        if chunk.len() == st.vad.config.frame_size {
                            let vad_state = st.vad.process_frame(chunk);
                            if vad_state == VadState::SpeechEnded {
                                speech_ended = true;
                            }
                        }
                    }

                    let app = app_weak.unwrap();
                    match st.vad.state() {
                        VadState::Speaking => app.set_vad_state("speaking".into()),
                        VadState::SpeechEnded => app.set_vad_state("speech_ended".into()),
                        VadState::Silent => app.set_vad_state("silent".into()),
                    }

                    if !speech_ended {
                        return;
                    }

                    // Speech ended — stop recording and process
                    let engine = st.engine.clone();
                    let recorder = st.recorder.take();
                    let auto_speak = st.auto_speak;
                    st.recording_started = None;
                    st.vad_active = false;
                    app.set_is_busy(true);
                    app.set_status_text("处理中...".into());
                    app.set_status_type("processing".into());

                    let Some(engine) = engine else {
                        app.set_status_text("引擎未初始化".into());
                        app.set_status_type("error".into());
                        app.set_is_busy(false);
                        return;
                    };
                    let Some(recorder) = recorder else {
                        app.set_is_busy(false);
                        return;
                    };

                    let audio = match recorder.into_wav_bytes() {
                        Ok(a) => a,
                        Err(e) => {
                            app.set_status_text(slint::format!("录音失败: {}", e));
                            app.set_status_type("error".into());
                            app.set_is_busy(false);
                            return;
                        }
                    };

                    // Get image (screen or camera)
                    let image_data: Option<Vec<u8>> = {
                        #[cfg(not(target_os = "android"))]
                        {
                            st.screen_capture
                                .as_ref()
                                .and_then(|sc| sc.latest_frame_jpeg())
                        }
                        #[cfg(target_os = "android")]
                        {
                            st.camera.as_ref().and_then(|cam| cam.latest_frame_jpeg(80))
                        }
                    };

                    drop(st);

                    // Transcribe audio
                    let transcription = {
                        let eng = engine.lock().await;
                        eng.transcribe(&audio).await
                    };

                    let app = app_weak.unwrap();

                    let question = match transcription {
                        Ok(ref text) => {
                            app.set_transcription(text.clone().into());
                            text.clone()
                        }
                        Err(ref e) => {
                            app.set_transcription(slint::format!("转写失败: {}", e));
                            app.set_is_busy(false);
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());
                            let mut st = state.lock().await;
                            start_continuous_listening(&mut st, &app);
                            return;
                        }
                    };

                    handle_question_response(
                        &state,
                        &app,
                        &app_weak,
                        engine.clone(),
                        question,
                        image_data,
                        auto_speak,
                    )
                    .await;

                    app.set_is_busy(false);

                    // Restart listening
                    let mut st = state.lock().await;
                    start_continuous_listening(&mut st, &app);
                })
                .unwrap();
            },
        );
    }

    // Text submitted
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_text_submitted(move |text| {
            let question: String = text.into();
            if question.is_empty() {
                return;
            }
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let engine = st.engine.clone();
                let auto_speak = st.auto_speak;
                drop(st);

                let Some(engine) = engine else { return };

                let app = app_weak.unwrap();
                app.set_transcription(question.clone().into());
                app.set_is_busy(true);
                app.set_status_text("分析中...".into());
                app.set_status_type("processing".into());

                // Get screen image
                #[cfg(not(target_os = "android"))]
                let image_data = {
                    let st = state.lock().await;
                    st.screen_capture
                        .as_ref()
                        .and_then(|sc| sc.latest_frame_jpeg())
                };
                #[cfg(target_os = "android")]
                let image_data: Option<Vec<u8>> = None;

                handle_question_response(
                    &state,
                    &app,
                    &app_weak,
                    engine.clone(),
                    question,
                    image_data,
                    auto_speak,
                )
                .await;

                let app = app_weak.unwrap();
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Confirm action
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_confirm_action(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let mut st = state.lock().await;
                if let Some(plan) = st.pending_plan.take() {
                    #[cfg(not(target_os = "android"))]
                    {
                        if let Some(ref mut ae) = st.automation {
                            match ae.execute_plan(&plan) {
                                Ok(result) => {
                                    let app = app_weak.unwrap();
                                    app.set_show_confirmation(false);
                                    app.set_status_text(slint::format!(
                                        "执行完成: {} 步",
                                        result.actions_executed
                                    ));
                                }
                                Err(e) => {
                                    let app = app_weak.unwrap();
                                    app.set_show_confirmation(false);
                                    app.set_status_text(slint::format!("执行失败: {}", e));
                                    app.set_status_type("error".into());
                                }
                            }
                        } else {
                            let app = app_weak.unwrap();
                            app.set_show_confirmation(false);
                            app.set_status_text("自动化引擎不可用".into());
                            app.set_status_type("error".into());
                        }
                    }
                    #[cfg(target_os = "android")]
                    {
                        let app = app_weak.unwrap();
                        app.set_show_confirmation(false);
                        app.set_status_text(slint::format!(
                            "Android 暂不支持执行 {} 个桌面自动化动作",
                            plan.actions.len()
                        ));
                    }
                }
            })
            .unwrap();
        });
    }

    // Cancel action
    {
        let app_weak = app_weak.clone();
        app.on_cancel_action(move || {
            let app = app_weak.unwrap();
            app.set_show_confirmation(false);
            app.set_confirmation_text("".into());
        });
    }

    // Open settings
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_open_settings(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let st = state.lock().await;
                let app = app_weak.unwrap();
                if let Some(ref engine) = st.engine {
                    let eng = engine.lock().await;
                    apply_config_to_app(&app, eng.config());
                }
                app.set_show_settings(true);
            })
            .unwrap();
        });
    }

    // Close settings
    {
        let app_weak = app_weak.clone();
        app.on_close_settings(move || {
            app_weak.unwrap().set_show_settings(false);
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
        app.on_max_tokens_changed(move |text| {
            app_weak.unwrap().set_max_tokens_str(text);
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                let config = {
                    let engine = engine.lock().await;
                    config_from_app(&app, engine.config())
                };

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
                        app.set_show_settings(false);
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                app.set_is_busy(true);

                let result = test_connection(engine).await;
                let app = app_weak.unwrap();

                match result {
                    Ok(true) => {
                        app.set_status_text("连接成功".into());
                        app.set_status_type("ready".into());
                    }
                    Ok(false) => {
                        app.set_status_text("连接失败".into());
                        app.set_status_type("error".into());
                    }
                    Err(e) => {
                        app.set_status_text(slint::format!("测试失败: {}", e));
                        app.set_status_type("error".into());
                    }
                }
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }
}

fn initialize_platform_services(st: &mut AppState) {
    #[cfg(not(target_os = "android"))]
    {
        let sc = screen_capture::ScreenCapture::new(screen_capture::CaptureConfig::default());
        if let Err(e) = sc.start() {
            tracing::warn!("Screen capture failed to start: {}", e);
        } else {
            st.screen_capture = Some(sc);
        }

        match automation::AutomationEngine::new(automation::AutomationConfig::default()) {
            Ok(ae) => st.automation = Some(ae),
            Err(e) => tracing::warn!("Automation engine failed: {}", e),
        }
    }

    #[cfg(target_os = "android")]
    {
        let cam = camera::AndroidCamera::new(camera::CameraConfig::default());
        if let Err(e) = cam.start() {
            tracing::warn!("Camera failed to start: {}", e);
        } else {
            st.camera = Some(cam);
        }
    }
}

fn start_continuous_listening(st: &mut AppState, app: &AppWindow) {
    match audio::Recorder::start() {
        Ok(recorder) => {
            st.recorder = Some(recorder);
            st.recording_started = Some(Instant::now());
            st.vad.reset();
            st.vad_active = true;
            app.set_vad_state("silent".into());
        }
        Err(e) => {
            app.set_status_text(slint::format!("麦克风启动失败: {}", e));
            app.set_status_type("error".into());
        }
    }
}

fn apply_config_to_app(app: &AppWindow, config: &AppConfig) {
    app.set_provider(config.cloud_api.provider.clone().into());
    app.set_api_key(config.cloud_api.api_key.clone().into());
    app.set_api_url(config.cloud_api.api_url.clone().into());
    app.set_model(config.cloud_api.model.clone().into());
    app.set_max_tokens_str(config.cloud_api.max_tokens.to_string().into());
    app.set_auto_speak(config.ui.auto_speak);
}

fn config_from_app(app: &AppWindow, base: &AppConfig) -> AppConfig {
    let mut config = base.clone();
    config.cloud_api.provider = app.get_provider().to_string();
    config.cloud_api.api_key = app.get_api_key().to_string();
    config.cloud_api.api_url = app
        .get_api_url()
        .to_string()
        .trim_end_matches('/')
        .to_string();
    config.cloud_api.model = app.get_model().to_string();
    if let Ok(budget) = app.get_max_tokens_str().to_string().parse::<usize>() {
        config.cloud_api.max_tokens = budget;
    }
    config.ui.auto_speak = app.get_auto_speak();
    config
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

async fn test_connection(engine: Arc<Mutex<AleEngine>>) -> Result<bool, String> {
    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    engine
        .test_cloud_api()
        .await
        .map_err(|error| error.to_string())
}

fn ensure_api_key(config: &AppConfig) -> Result<(), String> {
    if config.cloud_api.api_key.trim().is_empty() {
        return Err("API key 未配置".to_string());
    }
    Ok(())
}
