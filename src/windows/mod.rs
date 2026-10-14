mod capture;

use crate::{
    config::Config,
    pipeline::{Pipeline, Transcript},
};
use anyhow::{Result, ensure};
use capture::Capture;
use std::{
    mem::size_of,
    ptr::null_mut,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            KEYEVENTF_UNICODE, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, SendInput,
            UnregisterHotKey, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
        },
        WindowsAndMessaging::{
            GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, MSG,
            PM_REMOVE, PeekMessageW, WM_HOTKEY, WM_QUIT,
        },
    },
};

const TOGGLE: i32 = 1;
const CANCEL: i32 = 2;
const RECOVER: i32 = 3;

struct Hotkeys(Vec<i32>);

impl Hotkeys {
    fn register() -> Result<Self> {
        let mut keys = Self(vec![]);
        for (id, key) in [(TOGGLE, VK_SPACE), (CANCEL, VK_ESCAPE), (RECOVER, 0x56)] {
            // SAFETY: null HWND binds the hotkey to this thread's message queue.
            ensure!(
                unsafe {
                    RegisterHotKey(
                        null_mut(),
                        id,
                        MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                        key as u32,
                    )
                } != 0,
                "Cannot register Ctrl+Alt+{}. Another app or instance may own this shortcut.",
                match id {
                    TOGGLE => "Space",
                    CANCEL => "Esc",
                    _ => "V",
                }
            );
            keys.0.push(id);
        }
        Ok(keys)
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        for &id in &self.0 {
            // SAFETY: these IDs were registered by this thread.
            unsafe {
                UnregisterHotKey(null_mut(), id);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    window: HWND,
    focus: HWND,
}

impl Target {
    fn current() -> Option<Self> {
        // SAFETY: queried handles are not dereferenced; info has the required size.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let tid = GetWindowThreadProcessId(window, null_mut());
            let mut info: GUITHREADINFO = std::mem::zeroed();
            info.cbSize = size_of::<GUITHREADINFO>() as u32;
            if tid == 0 || GetGUIThreadInfo(tid, &mut info) == 0 || info.hwndFocus.is_null() {
                return None;
            }
            Some(Self {
                window,
                focus: info.hwndFocus,
            })
        }
    }
}

enum State {
    Idle,
    Recording {
        capture: Capture,
        target: Target,
        started: Instant,
        focus_changed: bool,
    },
    Processing {
        target: Target,
        focus_changed: bool,
        canceled: bool,
    },
}

pub fn run(config: Config) -> Result<()> {
    let pipeline = Arc::new(Pipeline::new(config.clone())?);
    let _hotkeys = Hotkeys::register()?;
    let (tx, rx) = mpsc::channel::<Result<Transcript>>();
    let mut state = State::Idle;
    let mut recovery: Option<String> = None;
    eprintln!(
        "Rust Dictate — Ctrl+Alt+Space: start/stop; Ctrl+Alt+Esc: cancel/clear; Ctrl+Alt+V: insert retained text."
    );
    eprintln!(
        "Audio is sent to {}. Cleanup: {}. Close this console to exit.",
        config.base_url, config.cleanup
    );
    eprintln!(
        "Ready. Focus an editable text field before starting. Do not change fields until insertion finishes."
    );
    loop {
        if let State::Recording {
            target,
            focus_changed,
            ..
        }
        | State::Processing {
            target,
            focus_changed,
            ..
        } = &mut state
        {
            *focus_changed |= Target::current() != Some(*target);
        }
        let mut message: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: message is a valid writable MSG; no window procedure is installed.
        while unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if message.message == WM_QUIT {
                return Ok(());
            }
            if message.message != WM_HOTKEY {
                continue;
            }
            match message.wParam as i32 {
                CANCEL => {
                    recovery = None;
                    state = match state {
                        State::Processing {
                            target,
                            focus_changed,
                            ..
                        } => {
                            eprintln!(
                                "Canceled. Waiting for in-flight request to finish; its result will be discarded."
                            );
                            State::Processing {
                                target,
                                focus_changed,
                                canceled: true,
                            }
                        }
                        _ => {
                            eprintln!("Canceled; audio and retained text discarded.");
                            State::Idle
                        }
                    };
                }
                TOGGLE => match state {
                    State::Idle => {
                        if let Some(target) = Target::current() {
                            match Capture::start(config.max_recording_seconds) {
                                Ok(capture) => {
                                    state = State::Recording {
                                        capture,
                                        target,
                                        started: Instant::now(),
                                        focus_changed: false,
                                    };
                                    eprintln!(
                                        "Recording… Ctrl+Alt+Space to finish, Ctrl+Alt+Esc to discard."
                                    );
                                }
                                Err(error) => eprintln!("Microphone error: {error:#}"),
                            }
                        } else {
                            eprintln!("Focus a text field first.");
                        }
                    }
                    State::Recording { .. } => finish_recording(&mut state, &pipeline, &tx),
                    State::Processing { .. } => eprintln!("Still processing; wait or cancel."),
                },
                RECOVER if matches!(state, State::Idle) => {
                    if let Some(text) = &recovery {
                        if let Some(target) = Target::current() {
                            match insert(text, target) {
                                Ok(()) => {
                                    recovery = None;
                                    eprintln!("Retained text inserted.");
                                }
                                Err(error) => eprintln!("Insertion failed: {error:#}"),
                            }
                        }
                    } else {
                        eprintln!("No retained transcript.");
                    }
                }
                _ => {}
            }
        }
        if let State::Recording {
            capture, started, ..
        } = &state
        {
            if capture.failed() {
                state = State::Idle;
                eprintln!("Microphone failed; recording discarded. Check your input device.");
            } else if capture.is_full()
                || started.elapsed().as_secs() >= config.max_recording_seconds as u64
            {
                eprintln!("Recording limit reached.");
                finish_recording(&mut state, &pipeline, &tx);
            }
        }
        if let Ok(result) = rx.try_recv() {
            let previous = std::mem::replace(&mut state, State::Idle);
            if let State::Processing {
                target,
                focus_changed,
                canceled,
            } = previous
            {
                if canceled {
                    eprintln!("Ready.");
                    continue;
                }
                match result {
                    Ok(transcript) => {
                        if transcript.cleanup_failed {
                            eprintln!("Cleanup failed; keeping original transcription.");
                        }
                        let insertion = if focus_changed {
                            Err(anyhow::anyhow!("Focus changed during dictation"))
                        } else {
                            insert(&transcript.text, target)
                        };
                        match insertion {
                            Ok(()) => {
                                recovery = None;
                                eprintln!("Inserted. Ready.");
                            }
                            Err(error) => {
                                recovery = Some(transcript.text);
                                eprintln!(
                                    "{error:#}. Text retained in memory. Focus the intended field and press Ctrl+Alt+V to retry, or Ctrl+Alt+Esc to discard. If any text was inserted, remove it before retrying."
                                );
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("Transcription failed: {error:#}. Ready to record again.")
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(15));
    }
}

fn finish_recording(
    state: &mut State,
    pipeline: &Arc<Pipeline>,
    tx: &mpsc::Sender<Result<Transcript>>,
) {
    let State::Recording {
        capture,
        target,
        focus_changed,
        ..
    } = std::mem::replace(state, State::Idle)
    else {
        return;
    };
    match capture.finish() {
        Ok(wav) => {
            let pipeline = Arc::clone(pipeline);
            let tx = tx.clone();
            *state = State::Processing {
                target,
                focus_changed,
                canceled: false,
            };
            thread::spawn(move || {
                let _ = tx.send(pipeline.transcribe(wav, "dictation.wav"));
            });
            eprintln!("Transcribing…");
        }
        Err(error) => eprintln!("Recording discarded: {error:#}"),
    }
}

fn insert(text: &str, target: Target) -> Result<()> {
    // Let the user release the shortcut before synthesizing ordinary text.
    let started = Instant::now();
    loop {
        ensure!(
            unsafe { GetAsyncKeyState(VK_ESCAPE as i32) } >= 0,
            "Insertion canceled"
        );
        ensure!(
            Target::current() == Some(target),
            "Target field changed; insertion skipped"
        );
        let held = [
            VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN, VK_SPACE, 0x56,
        ]
        .iter()
        .any(|&key| unsafe { GetAsyncKeyState(key as i32) } < 0);
        if !held {
            break;
        }
        ensure!(
            started.elapsed() < Duration::from_secs(2),
            "Release the shortcut keys before insertion"
        );
        thread::sleep(Duration::from_millis(10));
    }
    // Unicode events preserve the clipboard and do not synthesize Enter shortcuts.
    let units: Vec<_> = text.encode_utf16().collect();
    ensure!(
        units.len() <= 16_000,
        "Transcript too long for one insertion"
    );
    ensure!(
        !text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r'),
        "Transcript contains unsupported control characters"
    );
    let inputs: Vec<_> = units
        .into_iter()
        .flat_map(|unit| {
            [0, KEYEVENTF_KEYUP].map(|up| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: unit,
                        dwFlags: KEYEVENTF_UNICODE | up,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            })
        })
        .collect();
    ensure!(
        Target::current() == Some(target),
        "Target field changed; insertion skipped"
    );
    // SAFETY: inputs contains initialized INPUT structures and lives through SendInput.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    ensure!(
        sent == inputs.len() as u32,
        "Windows accepted {sent}/{} input events; the target may be elevated",
        inputs.len()
    );
    Ok(())
}
