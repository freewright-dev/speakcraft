use crate::audio::{MAX_AUDIO_BYTES, downsample, encode_wav};
use anyhow::{Context, Result, bail, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct Capture {
    /// None when a test build reads audio from a file instead of a device.
    stream: Option<cpal::Stream>,
    samples: Arc<Mutex<Vec<i16>>>,
    failed: Arc<AtomicBool>,
    full: Arc<AtomicBool>,
    sample_rate: u32,
    pub device: String,
}

/// Names of the input devices Windows currently offers.
pub fn microphones() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

/// The named device, else the Windows default, else the first available input:
/// Windows can report no default while a usable microphone is connected.
fn select_device(name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        return host
            .input_devices()?
            .find(|device| device.name().is_ok_and(|device| device == name))
            .with_context(|| {
                format!("Microphone \"{name}\" is not connected. Choose another in Saykey or Windows Sound settings.")
            });
    }
    host.default_input_device()
        .or_else(|| host.input_devices().ok()?.next())
        .context("No microphone found. Connect one and check Windows Sound settings.")
}

impl Capture {
    pub fn start(seconds: u32, microphone: Option<&str>) -> Result<Self> {
        #[cfg(feature = "test-audio")]
        if let Some(path) = std::env::var_os("SAYKEY_TEST_AUDIO") {
            return Self::from_file(path.as_ref(), seconds);
        }
        let device = select_device(microphone)?;
        // Try 16 kHz mono first, then the device default. Speech recognition
        // resamples to 16 kHz anyway, and a single channel keeps the upload
        // small, but many WASAPI shared-mode drivers expose only their mix
        // format and reject anything else, so the default is the fallback.
        let candidates = candidate_configs(&device);
        ensure!(
            !candidates.is_empty(),
            "Cannot open microphone. Check Windows microphone privacy permissions."
        );
        let mut last_error = None;
        for supported in candidates {
            match Self::try_start(&device, supported, seconds) {
                Ok(capture) => return Ok(capture),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Cannot open microphone. Check Windows microphone privacy permissions.")
        }))
    }

    fn try_start(
        device: &cpal::Device,
        supported: cpal::SupportedStreamConfig,
        seconds: u32,
    ) -> Result<Self> {
        let name = device.name().unwrap_or_else(|_| "microphone".into());
        let config: cpal::StreamConfig = supported.clone().into();
        ensure!(
            config.channels > 0 && config.sample_rate.0 > 0,
            "Invalid microphone format"
        );
        let limit =
            (config.sample_rate.0 as usize * seconds as usize).min((MAX_AUDIO_BYTES - 44) / 2 - 1);
        let samples = Arc::new(Mutex::new(Vec::with_capacity(limit)));
        let failed = Arc::new(AtomicBool::new(false));
        let full = Arc::new(AtomicBool::new(false));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                build::<f32>(device, &config, &samples, &failed, &full, limit)
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(device, &config, &samples, &failed, &full, limit)
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(device, &config, &samples, &failed, &full, limit)
            }
            format => bail!("Unsupported microphone sample format: {format}"),
        }?;
        stream.play().context("Cannot start microphone")?;
        Ok(Self {
            stream: Some(stream),
            samples,
            failed,
            full,
            sample_rate: config.sample_rate.0,
            device: name,
        })
    }

    /// End-to-end tests on machines without a microphone: replay a WAV file
    /// as if it had been captured. Only compiled with the `test-audio` feature.
    #[cfg(feature = "test-audio")]
    fn from_file(path: &std::path::Path, seconds: u32) -> Result<Self> {
        let mut reader = hound::WavReader::open(path).context("Cannot open SAYKEY_TEST_AUDIO")?;
        let spec = reader.spec();
        ensure!(
            spec.channels > 0 && spec.bits_per_sample == 16,
            "SAYKEY_TEST_AUDIO must be 16-bit PCM"
        );
        let interleaved = reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?;
        let limit = spec.sample_rate as usize * seconds as usize;
        let samples: Vec<i16> = interleaved
            .chunks_exact(spec.channels as usize)
            .take(limit)
            .map(|frame| (frame.iter().map(|&s| s as i32).sum::<i32>() / frame.len() as i32) as i16)
            .collect();
        Ok(Self {
            stream: None,
            samples: Arc::new(Mutex::new(samples)),
            failed: Arc::new(AtomicBool::new(false)),
            full: Arc::new(AtomicBool::new(false)),
            sample_rate: spec.sample_rate,
            device: format!("test audio {}", path.display()),
        })
    }

    pub fn is_full(&self) -> bool {
        self.full.load(Ordering::Relaxed)
    }

    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    pub fn finish(self) -> Result<Vec<u8>> {
        drop(self.stream);
        ensure!(
            !self.failed.load(Ordering::Relaxed),
            "Microphone disconnected or capture failed; recording discarded"
        );
        let samples = self
            .samples
            .lock()
            .map_err(|_| anyhow::anyhow!("Audio buffer unavailable"))?;
        let (samples, sample_rate) = downsample(&samples, self.sample_rate);
        encode_wav(&samples, sample_rate)
    }
}

/// Capture configurations to try, best first: 16 kHz mono when the hardware
/// offers it, then the device default. Echo cancellation, noise suppression,
/// and auto gain stay at the Windows defaults; cpal exposes no control over
/// them, so they are left to the OS rather than silently assumed off.
fn candidate_configs(device: &cpal::Device) -> Vec<cpal::SupportedStreamConfig> {
    const TARGET_RATE: u32 = 16_000;
    let mut configs = Vec::new();
    if let Ok(ranges) = device.supported_input_configs() {
        let mono_16k = ranges
            .filter(|range| {
                range.channels() == 1
                    && matches!(
                        range.sample_format(),
                        cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                    )
                    && range.min_sample_rate().0 <= TARGET_RATE
                    && range.max_sample_rate().0 >= TARGET_RATE
            })
            .min_by_key(|range| range.min_sample_rate().0)
            .map(|range| range.with_sample_rate(cpal::SampleRate(TARGET_RATE)));
        if let Some(config) = mono_16k {
            configs.push(config);
        }
    }
    if let Ok(default) = device.default_input_config() {
        configs.push(default);
    }
    configs
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: &Arc<Mutex<Vec<i16>>>,
    failed: &Arc<AtomicBool>,
    full: &Arc<AtomicBool>,
    limit: usize,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let samples = Arc::clone(samples);
    let failed_data = Arc::clone(failed);
    let failed = Arc::clone(failed);
    let full = Arc::clone(full);
    let channels = config.channels as usize;
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let Ok(mut samples) = samples.lock() else {
                failed_data.store(true, Ordering::Relaxed);
                return;
            };
            let remaining = limit - samples.len();
            for frame in data.chunks_exact(channels).take(remaining) {
                let mono = frame
                    .iter()
                    .map(|&sample| sample.to_sample::<f32>())
                    .sum::<f32>()
                    / channels as f32;
                samples.push((mono.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16);
            }
            if samples.len() == limit {
                full.store(true, Ordering::Relaxed);
            }
        },
        move |_| failed.store(true, Ordering::Relaxed),
        None,
    )?)
}
