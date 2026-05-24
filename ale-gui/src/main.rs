use ale_core::config::AppConfig;
use ale_core::{AleEngine, AleEngineFactory};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use iced::keyboard::{key::Named, Key};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{event, keyboard, time, Alignment, Element, Length, Subscription, Task};
use std::io::Cursor;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub fn main() -> iced::Result {
    iced::application(AleApp::new, AleApp::update, AleApp::view)
        .title("Ale, My Eyes!")
        .subscription(AleApp::subscription)
        .run()
}

struct AleApp {
    engine: Option<Arc<Mutex<AleEngine>>>,
    screen: Screen,
    status: AppStatus,
    settings: SettingsDraft,
    result: Option<AppResult>,
    recorder: Option<Recorder>,
    recording_started: Option<Instant>,
    last_image_info: Option<String>,
    show_api_key: bool,
    auto_speak: bool,
    diagnostics: DiagnosticsInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Main,
    Settings,
    Diagnostics,
}

#[derive(Debug, Clone)]
enum AppStatus {
    Initializing,
    Ready,
    Recording,
    Processing(&'static str),
    Error(String),
}

#[derive(Debug, Clone)]
struct AppResult {
    kind: ResultKind,
    text: String,
    metadata: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum ResultKind {
    Transcription,
    ImageDescription,
    ConnectionTest,
}

#[derive(Debug, Clone)]
struct SettingsDraft {
    provider: String,
    api_key: String,
    api_url: String,
    model: String,
    language: String,
    font_size: String,
    high_contrast: bool,
}

#[derive(Debug, Clone, Default)]
struct DiagnosticsInfo {
    api_url: String,
    model: String,
    recent_errors: Vec<String>,
}

#[derive(Clone)]
enum Message {
    EngineReady(Result<(Arc<Mutex<AleEngine>>, AppConfig), String>),
    ShowMain,
    ShowSettings,
    ShowDiagnostics,
    ProviderChanged(String),
    ApiKeyChanged(String),
    ApiUrlChanged(String),
    ModelChanged(String),
    LanguageChanged(String),
    FontSizeChanged(String),
    HighContrastChanged(bool),
    AutoSpeakChanged(bool),
    ToggleApiKeyVisible,
    SaveSettings,
    SettingsSaved(Result<(Arc<Mutex<AleEngine>>, AppConfig), String>),
    TestConnection,
    ConnectionTestFinished(Result<bool, String>),
    ToggleRecording,
    Tick,
    TranscriptionFinished(Result<String, String>),
    DescribeImage,
    ImageDescriptionFinished(Result<AppResult, String>),
    SpeakResult,
    ResultSpoken(Result<(), String>),
    ClearResult,
    ClearError,
    KeyPressed(keyboard::Event),
    StatusSpoken(()),
}

impl AleApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                engine: None,
                screen: Screen::Main,
                status: AppStatus::Initializing,
                settings: SettingsDraft::default(),
                result: None,
                recorder: None,
                recording_started: None,
                last_image_info: None,
                show_api_key: false,
                auto_speak: true,
                diagnostics: DiagnosticsInfo::default(),
            },
            Task::perform(create_engine(), Message::EngineReady),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EngineReady(Ok((engine, config))) => {
                self.engine = Some(engine);
                self.settings = SettingsDraft::from_config(&config);
                self.diagnostics = DiagnosticsInfo {
                    api_url: config.cloud_api.api_url.clone(),
                    model: config.cloud_api.model.clone(),
                    recent_errors: Vec::new(),
                };
                self.status = AppStatus::Ready;
                self.auto_speak_when_ready("初始化完成")
            }
            Message::EngineReady(Err(error)) => {
                self.status = AppStatus::Error(format!("初始化失败: {error}"));
                self.diagnostics.recent_errors.push(error.clone());
                Task::none()
            }
            Message::ShowMain => {
                self.screen = Screen::Main;
                Task::none()
            }
            Message::ShowSettings => {
                self.screen = Screen::Settings;
                Task::none()
            }
            Message::ShowDiagnostics => {
                self.screen = Screen::Diagnostics;
                Task::none()
            }
            Message::ProviderChanged(value) => {
                self.settings.provider = value;
                Task::none()
            }
            Message::ApiKeyChanged(value) => {
                self.settings.api_key = value;
                Task::none()
            }
            Message::ApiUrlChanged(value) => {
                self.settings.api_url = value;
                Task::none()
            }
            Message::ModelChanged(value) => {
                self.settings.model = value;
                Task::none()
            }
            Message::LanguageChanged(value) => {
                self.settings.language = value;
                Task::none()
            }
            Message::FontSizeChanged(value) => {
                self.settings.font_size = value;
                Task::none()
            }
            Message::HighContrastChanged(value) => {
                self.settings.high_contrast = value;
                Task::none()
            }
            Message::AutoSpeakChanged(value) => {
                self.auto_speak = value;
                Task::none()
            }
            Message::SaveSettings => {
                let Some(engine) = self.engine.clone() else {
                    self.status = AppStatus::Error("引擎尚未初始化，无法保存设置".to_string());
                    return Task::none();
                };

                let config = match self.settings.to_config() {
                    Ok(config) => config,
                    Err(error) => {
                        self.status = AppStatus::Error(error);
                        return Task::none();
                    }
                };

                self.status = AppStatus::Processing("正在保存设置...");
                Task::perform(save_settings(engine, config), Message::SettingsSaved)
            }
            Message::SettingsSaved(Ok((engine, config))) => {
                self.engine = Some(engine);
                self.settings = SettingsDraft::from_config(&config);
                self.status = AppStatus::Ready;
                self.screen = Screen::Main;
                Task::none()
            }
            Message::SettingsSaved(Err(error)) => {
                self.status = AppStatus::Error(format!("保存设置失败: {error}"));
                Task::none()
            }
            Message::ToggleApiKeyVisible => {
                self.show_api_key = !self.show_api_key;
                Task::none()
            }
            Message::TestConnection => {
                let Some(engine) = self.engine.clone() else {
                    self.status = AppStatus::Error("引擎尚未初始化，无法测试连接".to_string());
                    return Task::none();
                };

                self.status = AppStatus::Processing("正在测试云端连接...");
                Task::perform(test_connection(engine), Message::ConnectionTestFinished)
            }
            Message::ConnectionTestFinished(Ok(true)) => {
                self.status = AppStatus::Ready;
                self.result = Some(AppResult {
                    kind: ResultKind::ConnectionTest,
                    text: "云端连接测试成功".to_string(),
                    metadata: None,
                });
                Task::none()
            }
            Message::ConnectionTestFinished(Ok(false)) => {
                self.status = AppStatus::Error("云端连接测试失败".to_string());
                Task::none()
            }
            Message::ConnectionTestFinished(Err(error)) => {
                self.status = AppStatus::Error(format!("云端连接测试失败: {error}"));
                Task::none()
            }
            Message::ToggleRecording => {
                if self.recorder.is_none() {
                    match Recorder::start() {
                        Ok(recorder) => {
                            self.recorder = Some(recorder);
                            self.recording_started = Some(Instant::now());
                            self.status = AppStatus::Recording;
                            return self.auto_speak_status("开始录音");
                        }
                        Err(error) => {
                            self.status = AppStatus::Error(error.clone());
                            self.diagnostics.recent_errors.push(error);
                            return Task::none();
                        }
                    }
                }

                let Some(engine) = self.engine.clone() else {
                    self.status = AppStatus::Error("引擎尚未初始化，无法转写录音".to_string());
                    self.recorder = None;
                    self.recording_started = None;
                    return Task::none();
                };

                let Some(recorder) = self.recorder.take() else {
                    self.status = AppStatus::Error("录音状态丢失，请重新开始录音".to_string());
                    self.recording_started = None;
                    return Task::none();
                };
                self.recording_started = None;
                let audio = match recorder.into_wav_bytes() {
                    Ok(audio) => audio,
                    Err(error) => {
                        self.status = AppStatus::Error(format!("录音保存失败: {error}"));
                        return Task::none();
                    }
                };

                self.status = AppStatus::Processing("正在转写语音...");
                Task::perform(
                    transcribe_audio(engine, audio),
                    Message::TranscriptionFinished,
                )
            }
            Message::Tick => {
                if self.recorder.is_some()
                    && self
                        .recording_started
                        .is_some_and(|started| started.elapsed() >= Duration::from_secs(60))
                {
                    return self.update(Message::ToggleRecording);
                }

                Task::none()
            }
            Message::TranscriptionFinished(Ok(text)) => {
                self.status = AppStatus::Ready;
                self.result = Some(AppResult {
                    kind: ResultKind::Transcription,
                    text: text.clone(),
                    metadata: None,
                });
                self.auto_speak_when_ready(&format!("识别完成: {text}"))
            }
            Message::TranscriptionFinished(Err(error)) => {
                self.status = AppStatus::Error(format!("语音识别失败: {error}"));
                self.diagnostics.recent_errors.push(error.clone());
                Task::none()
            }
            Message::DescribeImage => {
                let Some(engine) = self.engine.clone() else {
                    self.status = AppStatus::Error("引擎尚未初始化，无法描述图片".to_string());
                    return Task::none();
                };

                self.status = AppStatus::Processing("正在选择并描述图片...");
                Task::perform(describe_image(engine), Message::ImageDescriptionFinished)
            }
            Message::ImageDescriptionFinished(Ok(result)) => {
                self.status = AppStatus::Ready;
                self.last_image_info = result.metadata.clone();
                let speak_text = format!("图片描述完成: {}", result.text);
                self.result = Some(result);
                self.auto_speak_when_ready(&speak_text)
            }
            Message::ImageDescriptionFinished(Err(error)) => {
                self.status = AppStatus::Error(format!("图片描述失败: {error}"));
                self.diagnostics.recent_errors.push(error.clone());
                Task::none()
            }
            Message::ClearResult => {
                self.result = None;
                Task::none()
            }
            Message::SpeakResult => {
                let Some(engine) = self.engine.clone() else {
                    self.status = AppStatus::Error("引擎尚未初始化，无法朗读结果".to_string());
                    return Task::none();
                };
                let Some(result) = self.result.clone() else {
                    self.status = AppStatus::Error("没有可朗读的结果".to_string());
                    return Task::none();
                };

                self.status = AppStatus::Processing("正在朗读结果...");
                Task::perform(speak_text(engine, result.text), Message::ResultSpoken)
            }
            Message::ResultSpoken(Ok(())) => {
                self.status = AppStatus::Ready;
                Task::none()
            }
            Message::ResultSpoken(Err(error)) => {
                self.status = AppStatus::Error(format!("朗读结果失败: {error}"));
                Task::none()
            }
            Message::ClearError => {
                if matches!(self.status, AppStatus::Error(_)) {
                    self.status = AppStatus::Ready;
                }
                Task::none()
            }
            Message::KeyPressed(event) => {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    let cmd = modifiers.command();
                    match key.as_ref() {
                        Key::Character("r") if cmd => self.update(Message::ToggleRecording),
                        Key::Character("o") if cmd => {
                            if self.engine.is_some() && !self.is_busy() {
                                self.update(Message::DescribeImage)
                            } else {
                                Task::none()
                            }
                        }
                        Key::Character("l") if cmd => {
                            if self.result.is_some() && self.engine.is_some() && !self.is_busy() {
                                self.update(Message::SpeakResult)
                            } else {
                                Task::none()
                            }
                        }
                        Key::Character("s") if cmd && self.screen == Screen::Settings => {
                            self.update(Message::SaveSettings)
                        }
                        Key::Character("d") if cmd => self.update(Message::ShowDiagnostics),
                        Key::Named(Named::Escape) => {
                            if matches!(self.status, AppStatus::Error(_)) {
                                self.update(Message::ClearError)
                            } else if self.screen != Screen::Main {
                                self.update(Message::ShowMain)
                            } else {
                                Task::none()
                            }
                        }
                        _ => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            Message::StatusSpoken(_) => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Main => self.view_main(),
            Screen::Settings => self.view_settings(),
            Screen::Diagnostics => self.view_diagnostics(),
        }
    }

    fn view_main(&self) -> Element<'_, Message> {
        let busy = self.is_busy();
        let can_use = self.engine.is_some() && !busy;

        let record_label = if self.recorder.is_some() {
            "停止录音并转写"
        } else {
            "开始语音输入"
        };
        let recording_hint = self
            .recording_started
            .map(|started| format!("已录制 {} 秒，最长 60 秒", started.elapsed().as_secs()))
            .unwrap_or_else(|| "未在录音。".to_string());
        let image_hint = self.last_image_info.as_deref().unwrap_or("尚未选择图片。");

        let record_message = if self.engine.is_some() && !busy || self.recorder.is_some() {
            Some(Message::ToggleRecording)
        } else {
            None
        };

        let controls = row![
            button(text(record_label).size(18))
                .padding(16)
                .style(if self.recorder.is_some() {
                    button::danger
                } else {
                    button::primary
                })
                .on_press_maybe(record_message),
            button(text("选择图片并描述").size(18))
                .padding(16)
                .style(button::secondary)
                .on_press_maybe(can_use.then_some(Message::DescribeImage)),
            button(text("设置").size(18))
                .padding(16)
                .on_press_maybe((!busy).then_some(Message::ShowSettings)),
            button(text("诊断").size(18))
                .padding(16)
                .style(button::text)
                .on_press_maybe((!busy).then_some(Message::ShowDiagnostics)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let content =
            column![
                self.status_card(),
                container(
                    column![
                    text("主要操作").size(24),
                    controls,
                    text("请先在设置中填写 API key。录音最长 60 秒，停止后上传到云端转写。")
                        .size(14),
                    text("快捷键: Ctrl+R 录音 | Ctrl+O 图片 | Ctrl+L 朗读 | Ctrl+D 诊断 | Esc 返回")
                        .size(12),
                    text(recording_hint).size(14),
                    text(image_hint).size(14),
                ]
                    .spacing(12),
                )
                .padding(16)
                .style(container::rounded_box),
                self.result_card(),
            ]
            .spacing(24)
            .padding(24)
            .width(Length::Fill);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let busy = self.is_busy();

        let form = column![
            text("设置").size(28),
            text("云端 API").size(20),
            text("当前后端按 OpenAI 兼容接口调用，Provider 建议保持 openai。").size(14),
            labeled_input(
                "Provider",
                "openai",
                &self.settings.provider,
                Message::ProviderChanged,
                false,
            ),
            column![
                text("API Key").size(14),
                row![
                    text_input("sk-...", &self.settings.api_key)
                        .secure(!self.show_api_key)
                        .on_input(Message::ApiKeyChanged)
                        .padding(10),
                    button(
                        text(if self.show_api_key {
                            "隐藏"
                        } else {
                            "显示"
                        })
                        .size(14)
                    )
                    .padding(10)
                    .on_press(Message::ToggleApiKeyVisible),
                ]
                .spacing(8),
            ]
            .spacing(4),
            labeled_input(
                "API URL",
                "https://api.openai.com/v1",
                &self.settings.api_url,
                Message::ApiUrlChanged,
                false,
            ),
            labeled_input(
                "Model",
                "gpt-4o",
                &self.settings.model,
                Message::ModelChanged,
                false,
            ),
            text("界面").size(20),
            labeled_input(
                "Language",
                "zh-CN",
                &self.settings.language,
                Message::LanguageChanged,
                false,
            ),
            labeled_input(
                "Font Size",
                "16",
                &self.settings.font_size,
                Message::FontSizeChanged,
                false,
            ),
            checkbox(self.settings.high_contrast)
                .label("高对比模式")
                .on_toggle(Message::HighContrastChanged),
            checkbox(self.auto_speak)
                .label("自动语音播报")
                .on_toggle(Message::AutoSpeakChanged),
            row![
                button(text("保存设置").size(16))
                    .padding(12)
                    .style(button::primary)
                    .on_press_maybe((!busy).then_some(Message::SaveSettings)),
                button(text("测试连接").size(16))
                    .padding(12)
                    .style(button::secondary)
                    .on_press_maybe(
                        (!busy && self.engine.is_some()).then_some(Message::TestConnection)
                    ),
                button(text("返回主界面").size(16))
                    .padding(12)
                    .on_press_maybe((!busy).then_some(Message::ShowMain)),
            ]
            .spacing(12),
            self.status_card(),
        ]
        .spacing(12)
        .padding(24)
        .width(Length::Fill);

        container(scrollable(form))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn status_card(&self) -> Element<'_, Message> {
        let mut content =
            column![text("系统状态").size(20), text(self.status_text()).size(16)].spacing(8);

        if matches!(self.status, AppStatus::Error(_)) {
            content = content.push(
                button(text("清除错误").size(14))
                    .padding(8)
                    .style(button::text)
                    .on_press(Message::ClearError),
            );
        }

        container(content)
            .padding(16)
            .width(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    fn result_card(&self) -> Element<'_, Message> {
        let Some(result) = &self.result else {
            return container(text("暂无结果").size(16))
                .padding(16)
                .width(Length::Fill)
                .style(container::rounded_box)
                .into();
        };

        let title = match result.kind {
            ResultKind::Transcription => "语音识别结果",
            ResultKind::ImageDescription => "图像描述结果",
            ResultKind::ConnectionTest => "连接测试结果",
        };

        let metadata = result.metadata.as_deref().unwrap_or("无附加信息");

        container(
            column![
                text(title).size(20),
                text(metadata).size(14),
                text(&result.text).size(16),
                row![
                    button(text("朗读结果").size(14))
                        .padding(8)
                        .style(button::secondary)
                        .on_press_maybe(
                            (self.engine.is_some() && !self.is_busy())
                                .then_some(Message::SpeakResult)
                        ),
                    button(text("清除结果").size(14))
                        .padding(8)
                        .style(button::text)
                        .on_press(Message::ClearResult),
                ]
                .spacing(8),
            ]
            .spacing(8),
        )
        .padding(16)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
    }

    fn status_text(&self) -> String {
        match &self.status {
            AppStatus::Initializing => "正在初始化...".to_string(),
            AppStatus::Ready => "就绪".to_string(),
            AppStatus::Recording => "正在录音，再次按下按钮结束录音".to_string(),
            AppStatus::Processing(message) => (*message).to_string(),
            AppStatus::Error(error) => error.clone(),
        }
    }

    fn is_busy(&self) -> bool {
        matches!(
            self.status,
            AppStatus::Initializing | AppStatus::Processing(_)
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(keyboard_event) => Some(Message::KeyPressed(keyboard_event)),
            _ => None,
        });

        let tick_sub = if self.recorder.is_some() {
            time::every(Duration::from_secs(1)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };

        Subscription::batch(vec![keyboard_sub, tick_sub])
    }

    fn view_diagnostics(&self) -> Element<'_, Message> {
        let engine_status = if self.engine.is_some() {
            "已初始化"
        } else {
            "未初始化"
        };

        let errors_text = if self.diagnostics.recent_errors.is_empty() {
            "无".to_string()
        } else {
            self.diagnostics.recent_errors.join("\n")
        };

        let content = column![
            text("诊断信息").size(28),
            text(format!("引擎状态: {engine_status}")).size(16),
            text(format!("API URL: {}", self.diagnostics.api_url)).size(16),
            text(format!("模型: {}", self.diagnostics.model)).size(16),
            text(format!(
                "自动语音: {}",
                if self.auto_speak { "开启" } else { "关闭" }
            ))
            .size(16),
            text(format!(
                "高对比模式: {}",
                if self.settings.high_contrast {
                    "开启"
                } else {
                    "关闭"
                }
            ))
            .size(16),
            text("最近错误:").size(16),
            text(errors_text).size(14),
            button(text("返回主界面").size(16))
                .padding(12)
                .on_press(Message::ShowMain),
        ]
        .spacing(12)
        .padding(24)
        .width(Length::Fill);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn auto_speak_when_ready(&mut self, text: &str) -> Task<Message> {
        if self.auto_speak && self.engine.is_some() {
            Task::perform(
                speak_text_if_available(self.engine.clone(), text.to_string()),
                |_| Message::StatusSpoken(()),
            )
        } else {
            Task::none()
        }
    }

    fn auto_speak_status(&self, text: &str) -> Task<Message> {
        if self.auto_speak && self.engine.is_some() {
            if let Some(engine) = self.engine.clone() {
                Task::perform(speak_text(engine, text.to_string()), |_| {
                    Message::StatusSpoken(())
                })
            } else {
                Task::none()
            }
        } else {
            Task::none()
        }
    }
}

impl Default for SettingsDraft {
    fn default() -> Self {
        Self::from_config(&AppConfig::default())
    }
}

impl SettingsDraft {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            provider: config.cloud_api.provider.clone(),
            api_key: config.cloud_api.api_key.clone(),
            api_url: config.cloud_api.api_url.clone(),
            model: config.cloud_api.model.clone(),
            language: config.ui.language.clone(),
            font_size: config.ui.font_size.to_string(),
            high_contrast: config.ui.high_contrast,
        }
    }

    fn to_config(&self) -> Result<AppConfig, String> {
        let font_size = self
            .font_size
            .trim()
            .parse::<u32>()
            .map_err(|_| "字体大小必须是数字".to_string())?;

        if self.provider.trim().is_empty() {
            return Err("Provider 不能为空".to_string());
        }

        if self.api_url.trim().is_empty() {
            return Err("API URL 不能为空".to_string());
        }

        if self.model.trim().is_empty() {
            return Err("模型名称不能为空".to_string());
        }

        let mut config = AppConfig::default();
        config.cloud_api.provider = self.provider.trim().to_string();
        config.cloud_api.api_key = self.api_key.trim().to_string();
        config.cloud_api.api_url = self.api_url.trim().trim_end_matches('/').to_string();
        config.cloud_api.model = self.model.trim().to_string();
        config.ui.language = self.language.trim().to_string();
        config.ui.font_size = font_size;
        config.ui.high_contrast = self.high_contrast;

        Ok(config)
    }
}

struct Recorder {
    stream: cpal::Stream,
    samples: Arc<StdMutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl Recorder {
    fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "没有找到可用麦克风".to_string())?;
        let supported_config = device
            .default_input_config()
            .map_err(|error| format!("获取麦克风配置失败: {error}"))?;
        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        let samples = Arc::new(StdMutex::new(Vec::new()));

        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_input_stream::<f32>(&device, &config, samples.clone()),
            cpal::SampleFormat::I16 => build_input_stream::<i16>(&device, &config, samples.clone()),
            cpal::SampleFormat::U16 => build_input_stream::<u16>(&device, &config, samples.clone()),
            other => Err(format!("不支持的麦克风采样格式: {other:?}")),
        }?;

        stream
            .play()
            .map_err(|error| format!("启动录音失败: {error}"))?;

        Ok(Self {
            stream,
            samples,
            sample_rate,
            channels,
        })
    }

    fn into_wav_bytes(self) -> Result<Vec<u8>, String> {
        let Recorder {
            stream,
            samples,
            sample_rate,
            channels,
        } = self;
        drop(stream);

        let samples = samples
            .lock()
            .map_err(|_| "读取录音缓存失败".to_string())?
            .clone();

        if samples.is_empty() {
            return Err("没有录到音频".to_string());
        }

        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)
                .map_err(|error| format!("创建 WAV 失败: {error}"))?;
            for sample in samples {
                let sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer
                    .write_sample(sample)
                    .map_err(|error| format!("写入 WAV 失败: {error}"))?;
            }
            writer
                .finalize()
                .map_err(|error| format!("完成 WAV 失败: {error}"))?;
        }

        Ok(cursor.into_inner())
    }
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<StdMutex<Vec<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut buffer) = samples.lock() {
                    buffer.extend(data.iter().copied().map(f32::from_sample));
                }
            },
            move |error| {
                eprintln!("录音流错误: {error}");
            },
            None,
        )
        .map_err(|error| format!("创建录音流失败: {error}"))
}

trait FromSample<T> {
    fn from_sample(sample: T) -> f32;
}

impl FromSample<f32> for f32 {
    fn from_sample(sample: f32) -> f32 {
        sample
    }
}

impl FromSample<i16> for f32 {
    fn from_sample(sample: i16) -> f32 {
        sample as f32 / i16::MAX as f32
    }
}

impl FromSample<u16> for f32 {
    fn from_sample(sample: u16) -> f32 {
        (sample as f32 / u16::MAX as f32) * 2.0 - 1.0
    }
}

fn labeled_input<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    secure: bool,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        text_input(placeholder, value)
            .secure(secure)
            .on_input(on_input)
            .padding(10)
    ]
    .spacing(4)
    .into()
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

async fn describe_image(engine: Arc<Mutex<AleEngine>>) -> Result<AppResult, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
        .await
        .ok_or_else(|| "未选择图片".to_string())?;
    let path = file.path().to_path_buf();
    let bytes = tokio::fs::read(file.path())
        .await
        .map_err(|error| format!("读取图片失败: {error}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未知文件")
        .to_string();
    let metadata = format!("图片: {file_name}，大小: {} bytes", bytes.len());

    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    let text = engine
        .describe_image(&bytes)
        .await
        .map_err(|error| error.to_string())?;

    Ok(AppResult {
        kind: ResultKind::ImageDescription,
        text,
        metadata: Some(metadata),
    })
}

async fn transcribe_audio(engine: Arc<Mutex<AleEngine>>, audio: Vec<u8>) -> Result<String, String> {
    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    engine
        .transcribe(&audio)
        .await
        .map_err(|error| error.to_string())
}

fn ensure_api_key(config: &AppConfig) -> Result<(), String> {
    if config.cloud_api.api_key.trim().is_empty() {
        return Err("API key 未配置，请先打开设置填写".to_string());
    }

    Ok(())
}

async fn test_connection(engine: Arc<Mutex<AleEngine>>) -> Result<bool, String> {
    let engine = engine.lock().await;
    ensure_api_key(engine.config())?;
    engine
        .test_cloud_api()
        .await
        .map_err(|error| error.to_string())
}

async fn speak_text(engine: Arc<Mutex<AleEngine>>, text: String) -> Result<(), String> {
    let audio = {
        let engine = engine.lock().await;
        ensure_api_key(engine.config())?;
        engine
            .synthesize(&text)
            .await
            .map_err(|error| error.to_string())?
    };

    tokio::task::spawn_blocking(move || play_audio(audio))
        .await
        .map_err(|error| format!("音频播放任务失败: {error}"))?
}

async fn speak_text_if_available(
    engine: Option<Arc<Mutex<AleEngine>>>,
    text: String,
) -> Result<(), String> {
    let Some(engine) = engine else {
        return Ok(());
    };
    let audio = {
        let engine = engine.lock().await;
        if engine.config().cloud_api.api_key.trim().is_empty() {
            return Ok(());
        }
        match engine.synthesize(&text).await {
            Ok(audio) => audio,
            Err(_) => return Ok(()),
        }
    };

    tokio::task::spawn_blocking(move || {
        let _ = play_audio(audio);
    })
    .await
    .map_err(|error| format!("音频播放任务失败: {error}"))
}

fn play_audio(audio: Vec<u8>) -> Result<(), String> {
    let cursor = Cursor::new(audio);
    let source = rodio::Decoder::new(cursor).map_err(|error| format!("解析音频失败: {error}"))?;
    let (_stream, handle) =
        rodio::OutputStream::try_default().map_err(|error| format!("打开音频输出失败: {error}"))?;
    let sink = rodio::Sink::try_new(&handle).map_err(|error| format!("创建播放器失败: {error}"))?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
