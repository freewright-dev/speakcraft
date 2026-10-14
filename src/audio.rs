use anyhow::{Result, ensure};
use std::io::Cursor;

pub const MAX_AUDIO_BYTES: usize = 24_000_000;

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
