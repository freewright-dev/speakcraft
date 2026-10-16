use anyhow::{Result, ensure};
use std::{borrow::Cow, io::Cursor};

pub const MAX_AUDIO_BYTES: usize = 24_000_000;
/// Speech models work at 16 kHz; uploading more only adds size and latency.
pub const SPEECH_RATE: u32 = 16_000;

/// Downsample mono PCM above 16 kHz by averaging each output sample's span of
/// input samples (a box low-pass filter that also limits aliasing). Lower
/// rates are returned unchanged; upsampling would add data without detail.
pub fn downsample(samples: &[i16], sample_rate: u32) -> (Cow<'_, [i16]>, u32) {
    if sample_rate <= SPEECH_RATE {
        return (Cow::Borrowed(samples), sample_rate);
    }
    let ratio = sample_rate as f64 / SPEECH_RATE as f64;
    let len = (samples.len() as f64 / ratio) as usize;
    let output = (0..len)
        .map(|index| {
            let start = (index as f64 * ratio) as usize;
            let end = (((index + 1) as f64 * ratio) as usize).clamp(start + 1, samples.len());
            let sum: i64 = samples[start..end]
                .iter()
                .map(|&sample| sample as i64)
                .sum();
            (sum / (end - start) as i64) as i16
        })
        .collect();
    (Cow::Owned(output), SPEECH_RATE)
}

/// Encode mono PCM in memory. Never create a temporary recording on disk.
pub fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>> {
    ensure!(sample_rate > 0, "Invalid sample rate");
    ensure!(
        samples.len() >= sample_rate as usize / 5,
        "Recording is shorter than 200 ms"
    );
    ensure!(
        samples.iter().any(|sample| sample.unsigned_abs() > 4),
        "No audible input; check your microphone"
    );
    ensure!(
        samples.len() < (MAX_AUDIO_BYTES - 44) / 2,
        "Recording exceeds the 24 MB upload limit"
    );
    let mut bytes = Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    {
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for &sample in samples {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
    }
    Ok(bytes.into_inner())
}
