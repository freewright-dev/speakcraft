use crate::audio::{MAX_AUDIO_BYTES, encode_wav};
use anyhow::{Context, Result, bail, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct Capture {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    failed: Arc<AtomicBool>,
    full: Arc<AtomicBool>,
    sample_rate: u32,
}

impl Capture {
    pub fn start(seconds: u32) -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .context("No default microphone. Check Windows Sound settings.")?;
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
            stream,
            samples,
            failed,
            full,
            sample_rate: config.sample_rate.0,
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
        encode_wav(&samples, self.sample_rate)
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
