use std::io::Cursor;
use std::sync::{Arc, Mutex as StdMutex};

pub struct Recorder {
    #[cfg(not(target_os = "android"))]
    stream: cpal::Stream,
    samples: Arc<StdMutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl Recorder {
    pub fn start() -> Result<Self, String> {
        #[cfg(not(target_os = "android"))]
        {
            Self::start_desktop()
        }
        #[cfg(target_os = "android")]
        {
            Self::start_android()
        }
    }

    pub fn into_wav_bytes(self) -> Result<Vec<u8>, String> {
        let samples = self
            .samples
            .lock()
            .map_err(|_| "读取录音缓存失败".to_string())?
            .clone();

        if samples.is_empty() {
            return Err("没有录到音频".to_string());
        }

        #[cfg(not(target_os = "android"))]
        {
            drop(self.stream);
        }

        let spec = hound::WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
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

    /// Drain accumulated samples and return as i16 PCM bytes (for VAD processing)
    pub fn take_samples(&self) -> Vec<u8> {
        let Ok(mut buffer) = self.samples.lock() else {
            return Vec::new();
        };
        let samples: Vec<f32> = buffer.drain(..).collect();
        let mut pcm = Vec::with_capacity(samples.len() * 2);
        for s in samples {
            let i = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm.extend_from_slice(&i.to_le_bytes());
        }
        pcm
    }

    #[cfg(not(target_os = "android"))]
    fn start_desktop() -> Result<Self, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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

    #[cfg(target_os = "android")]
    fn start_android() -> Result<Self, String> {
        use oboe::{
            AudioInputCallback, AudioInputStream, AudioStreamBuilder, DataCallbackResult, Mono,
            PerformanceMode, SharingMode,
        };

        let samples = Arc::new(StdMutex::new(Vec::new()));
        let samples_clone = samples.clone();

        let stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Shared)
            .set_format::<f32>()
            .set_channel_count::<Mono>()
            .set_callback(RecorderCallback {
                samples: samples_clone,
            })
            .open_stream()
            .map_err(|error| format!("打开音频流失败: {error:?}"))?;

        stream
            .start()
            .map_err(|error| format!("启动录音失败: {error:?}"))?;

        // Leak the stream so it keeps running until stop
        let stream_ref = Box::new(stream);
        std::mem::forget(stream_ref);

        Ok(Self {
            samples,
            sample_rate: 48000,
            channels: 1,
        })
    }
}

#[cfg(target_os = "android")]
struct RecorderCallback {
    samples: Arc<StdMutex<Vec<f32>>>,
}

#[cfg(target_os = "android")]
impl AudioInputCallback for RecorderCallback {
    type FrameType = (f32, Mono);

    fn on_audio_ready(
        &mut self,
        _stream: &mut dyn AudioInputStream,
        frames: &[f32],
    ) -> DataCallbackResult {
        if let Ok(mut buffer) = self.samples.lock() {
            buffer.extend_from_slice(frames);
        }
        DataCallbackResult::Continue
    }
}

#[cfg(not(target_os = "android"))]
fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<StdMutex<Vec<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    use cpal::traits::DeviceTrait;

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
