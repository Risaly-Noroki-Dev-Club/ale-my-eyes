pub mod audio;
pub mod file_picker;
pub mod tts_player;

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
            diagnostics_errors: Vec::new(),
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
                    st.engine = Some(engine);
                    apply_config_to_app(&app, &config);
                    app.set_engine_ready(true);
                    app.set_engine_status("已初始化".into());
                    app.set_status_text("就绪".into());
                    app.set_status_type("ready".into());
                    app.set_api_url(config.cloud_api.api_url.into());
                    app.set_model(config.cloud_api.model.into());

                    // Desktop: start screen capture + automation
                    #[cfg(not(target_os = "android"))]
                    {
                        let sc = screen_capture::ScreenCapture::new(
                            screen_capture::CaptureConfig::default(),
                        );
                        if let Err(e) = sc.start() {
                            tracing::warn!("Screen capture failed to start: {}", e);
                        } else {
                            st.screen_capture = Some(sc);
                        }

                        match automation::AutomationEngine::new(
                            automation::AutomationConfig::default(),
                        ) {
                            Ok(ae) => st.automation = Some(ae),
                            Err(e) => tracing::warn!("Automation engine failed: {}", e),
                        }
                    }

                    // Android: start camera
                    #[cfg(target_os = "android")]
                    {
                        let cam = camera::AndroidCamera::new(camera::CameraConfig::default());
                        if let Err(e) = cam.start() {
                            tracing::warn!("Camera failed to start: {}", e);
                        } else {
                            st.camera = Some(cam);
                        }
                    }

                    // Start continuous recording + VAD
                    start_continuous_listening(&mut st, &app);
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

    // VAD processing timer — checks for speech end every 100ms
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

                    // Get accumulated audio samples and feed to VAD
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

                    // Update UI vad state
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
                    app.set_is_recording(false);
                    app.set_is_busy(true);
                    app.set_status_text("正在处理...".into());
                    app.set_status_type("processing".into());

                    let Some(engine) = engine else {
                        app.set_status_text("引擎尚未初始化".into());
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
                            st.camera.as_ref().and_then(|cam| {
                                cam.latest_frame().map(|f| {
                                    image::RgbaImage::from_raw(f.width, f.height, f.rgba_data)
                                        .map(|img| {
                                            image::DynamicImage::ImageRgba8(img).to_rgb8().to_vec()
                                        })
                                        .unwrap_or_default()
                                })
                            })
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
                            // Restart listening
                            let mut st = state.lock().await;
                            start_continuous_listening(&mut st, &app);
                            return;
                        }
                    };

                    // If we have an image, ask about it; otherwise just show transcription
                    if let Some(img) = image_data {
                        app.set_status_text("正在分析画面...".into());
                        let result = {
                            let eng = engine.lock().await;
                            eng.ask_about_image(&img, &question).await
                        };

                        match result {
                            Ok(response) => {
                                app.set_ai_response(response.content.clone().into());

                                // Parse tool calls into action plan
                                if let Some(ref calls) = response.tool_calls {
                                    if !calls.is_empty() {
                                        let steps: Vec<String> = calls
                                            .iter()
                                            .map(|tc| {
                                                format!(
                                                    "{}: {}",
                                                    tc.function.name, tc.function.arguments
                                                )
                                            })
                                            .collect();
                                        app.set_action_steps(steps.join("\n").into());
                                    }
                                }

                                app.set_status_text("就绪".into());
                                app.set_status_type("ready".into());

                                if auto_speak {
                                    let text = response.content.clone();
                                    let engine = engine.clone();
                                    let app_weak2 = app_weak.clone();
                                    slint::spawn_local(async move {
                                        let _ = speak_and_play(engine, &text).await;
                                        let app = app_weak2.unwrap();
                                        app.set_status_text("就绪".into());
                                        app.set_status_type("ready".into());
                                    })
                                    .unwrap();
                                }
                            }
                            Err(e) => {
                                app.set_ai_response(slint::format!("分析失败: {}", e));
                                app.set_status_text("就绪".into());
                                app.set_status_type("ready".into());
                            }
                        }
                    } else {
                        // No image, just show transcription
                        app.set_ai_response("".into());
                        app.set_status_text("就绪".into());
                        app.set_status_type("ready".into());

                        if auto_speak && !question.is_empty() {
                            let engine = engine.clone();
                            let app_weak2 = app_weak.clone();
                            slint::spawn_local(async move {
                                let _ = speak_and_play(engine, &question).await;
                                let app = app_weak2.unwrap();
                                app.set_status_text("就绪".into());
                                app.set_status_type("ready".into());
                            })
                            .unwrap();
                        }
                    }

                    app.set_is_busy(false);

                    // Restart listening
                    let mut st = state.lock().await;
                    start_continuous_listening(&mut st, &app);
                })
                .unwrap();
            },
        );
    }

    // Text submitted (desktop fallback input)
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
                app.set_status_text("正在分析...".into());
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

                drop(state.lock().await);

                if let Some(img) = image_data {
                    let result = {
                        let eng = engine.lock().await;
                        eng.ask_about_image(&img, &question).await
                    };

                    let app = app_weak.unwrap();
                    match result {
                        Ok(response) => {
                            app.set_ai_response(response.content.clone().into());
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());

                            if auto_speak {
                                let engine = engine.clone();
                                let app_weak2 = app_weak.clone();
                                let text = response.content.clone();
                                slint::spawn_local(async move {
                                    let _ = speak_and_play(engine, &text).await;
                                    let app = app_weak2.unwrap();
                                    app.set_status_text("就绪".into());
                                    app.set_status_type("ready".into());
                                })
                                .unwrap();
                            }
                        }
                        Err(e) => {
                            app.set_ai_response(slint::format!("失败: {}", e));
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());
                        }
                    }
                }

                let app = app_weak.unwrap();
                app.set_is_busy(false);
            })
            .unwrap();
        });
    }

    // Confirm action (desktop)
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_confirm_action(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let mut st = state.lock().await;
                if let Some(plan) = st.pending_plan.take() {
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
                            }
                        }
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

    // Toggle monitoring (restart VAD listening)
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_toggle_monitoring(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let mut st = state.lock().await;
                let app = app_weak.unwrap();
                if st.vad_active {
                    st.vad_active = false;
                    st.recorder = None;
                    st.recording_started = None;
                    app.set_is_recording(false);
                    app.set_vad_state("silent".into());
                } else {
                    start_continuous_listening(&mut st, &app);
                }
            })
            .unwrap();
        });
    }

    // Clear conversation
    {
        let app_weak = app_weak.clone();
        app.on_clear_conversation(move || {
            let app = app_weak.unwrap();
            app.set_transcription("".into());
            app.set_ai_response("".into());
            app.set_action_steps("".into());
        });
    }

    // Legacy callbacks (kept for mobile screen compatibility)
    {
        let state = state.clone();
        let app_weak = app_weak.clone();
        app.on_toggle_recording(move || {
            let state = state.clone();
            let app_weak = app_weak.clone();
            slint::spawn_local(async move {
                let mut st = state.lock().await;
                let app = app_weak.unwrap();

                if st.recorder.is_none() {
                    match audio::Recorder::start() {
                        Ok(recorder) => {
                            st.recorder = Some(recorder);
                            st.recording_started = Some(Instant::now());
                            st.vad.reset();
                            st.vad_active = true;
                            app.set_is_recording(true);
                            app.set_status_text("正在录音...".into());
                            app.set_status_type("recording".into());
                        }
                        Err(error) => {
                            app.set_status_text(slint::format!("{}", error));
                            app.set_status_type("error".into());
                        }
                    }
                } else {
                    // Manual stop
                    let engine = st.engine.clone();
                    let recorder = st.recorder.take();
                    let auto_speak = st.auto_speak;
                    st.recording_started = None;
                    st.vad_active = false;
                    app.set_is_recording(false);

                    let Some(engine) = engine else { return };
                    let Some(recorder) = recorder else { return };

                    let audio = match recorder.into_wav_bytes() {
                        Ok(a) => a,
                        Err(e) => {
                            app.set_status_text(slint::format!("录音失败: {}", e));
                            return;
                        }
                    };

                    app.set_status_text("正在转写...".into());
                    app.set_status_type("processing".into());
                    app.set_is_busy(true);
                    drop(st);

                    let result = transcribe_audio(engine.clone(), audio).await;
                    let app = app_weak.unwrap();

                    match result {
                        Ok(text) => {
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());
                            app.set_result_title("语音识别结果".into());
                            app.set_result_text(text.clone().into());
                            app.set_has_result(true);

                            if auto_speak {
                                let engine = engine.clone();
                                let app_weak2 = app_weak.clone();
                                slint::spawn_local(async move {
                                    let _ = speak_and_play(engine, &text).await;
                                    let app = app_weak2.unwrap();
                                    app.set_status_text("就绪".into());
                                    app.set_status_type("ready".into());
                                })
                                .unwrap();
                            }
                        }
                        Err(error) => {
                            app.set_status_text(slint::format!("转写失败: {}", error));
                            app.set_status_type("error".into());
                        }
                    }
                    app.set_is_busy(false);
                }
            })
            .unwrap();
        });
    }

    // Describe image (legacy, for mobile)
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                app.set_status_text("正在选择图片...".into());
                app.set_status_type("processing".into());
                app.set_is_busy(true);

                let result = describe_image_inner().await;

                if let Ok((bytes, file_name)) = result {
                    let metadata = format!("图片: {}", file_name);
                    let eng = engine.lock().await;
                    let text = eng.describe_image(&bytes).await;

                    let app = app_weak.unwrap();
                    match text {
                        Ok(desc) => {
                            app.set_result_title("图像描述".into());
                            app.set_result_text(desc.clone().into());
                            app.set_result_metadata(metadata.into());
                            app.set_has_result(true);
                            app.set_status_text("就绪".into());
                            app.set_status_type("ready".into());

                            if auto_speak {
                                let engine = engine.clone();
                                let app_weak2 = app_weak.clone();
                                slint::spawn_local(async move {
                                    let _ = speak_and_play(engine, &desc).await;
                                    let app = app_weak2.unwrap();
                                    app.set_status_text("就绪".into());
                                    app.set_status_type("ready".into());
                                })
                                .unwrap();
                            }
                        }
                        Err(e) => {
                            app.set_status_text(slint::format!("描述失败: {}", e));
                            app.set_status_type("error".into());
                        }
                    }
                }
                let app = app_weak.unwrap();
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                let result_text: String = app.get_result_text().into();
                if result_text.is_empty() {
                    return;
                }

                app.set_status_text("正在朗读...".into());
                app.set_is_busy(true);

                let _ = speak_and_play(engine, &result_text).await;
                let app = app_weak.unwrap();
                app.set_status_text("就绪".into());
                app.set_status_type("ready".into());
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                let config = config_from_app(&app);

                app.set_status_text("正在保存...".into());
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

                let Some(engine) = engine else { return };
                let app = app_weak.unwrap();
                app.set_status_text("测试中...".into());
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
}

fn start_continuous_listening(st: &mut AppState, app: &AppWindow) {
    match audio::Recorder::start() {
        Ok(recorder) => {
            st.recorder = Some(recorder);
            st.recording_started = Some(Instant::now());
            st.vad.reset();
            st.vad_active = true;
            app.set_is_recording(true);
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
        .map_err(|error| format!("播放失败: {error}"))?
}

fn ensure_api_key(config: &AppConfig) -> Result<(), String> {
    if config.cloud_api.api_key.trim().is_empty() {
        return Err("API key 未配置".to_string());
    }
    Ok(())
}
