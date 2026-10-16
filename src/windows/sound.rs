//! Short tones that make the recording state audible while another app has focus.

use std::{io::Cursor, ptr::null_mut, sync::OnceLock};
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

#[derive(Clone, Copy)]
pub enum Cue {
    Start,
    Stop,
    Cancel,
    Error,
}

pub fn play(cue: Cue) {
    static SOUNDS: OnceLock<[Vec<u8>; 4]> = OnceLock::new();
    let sounds = SOUNDS.get_or_init(|| {
        [
            tone(&[660.0, 880.0]),
            tone(&[880.0, 660.0]),
            tone(&[440.0]),
            tone(&[330.0, 247.0]),
        ]
    });
    let wav = &sounds[cue as usize];
    // SAFETY: SND_MEMORY reads a static WAV image that outlives asynchronous playback.
    unsafe {
        PlaySoundW(
            wav.as_ptr().cast(),
            null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

/// A quiet sequence of 70 ms notes with a sine envelope, so there are no clicks.
fn tone(notes: &[f32]) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let per_note = RATE as usize * 70 / 1000;
    let mut bytes = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut bytes,
        hound::WavSpec {
            channels: 1,
            sample_rate: RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .expect("in-memory WAV");
    for &frequency in notes {
        for index in 0..per_note {
            let progress = index as f32 / per_note as f32;
            let envelope = (std::f32::consts::PI * progress).sin();
            let phase = std::f32::consts::TAU * frequency * index as f32 / RATE as f32;
            let sample = phase.sin() * envelope * 0.2 * i16::MAX as f32;
            writer.write_sample(sample as i16).expect("in-memory WAV");
        }
    }
    writer.finalize().expect("in-memory WAV");
    bytes.into_inner()
}
