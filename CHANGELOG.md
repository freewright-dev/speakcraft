# Changelog

User-visible changes to SpeakCraft. Versions match `Cargo.toml`. Local build entries do not imply a published release, signed installer, or completed Windows desktop verification.

## Unreleased

### Added

- Hold-to-talk: holding Ctrl+Alt+Space for at least half a second records until release. A quick tap still toggles recording.
- Start, stop, cancel, and error sounds, plus an always-on-top status overlay that does not take focus or mouse input. Both can be turned off, and the overlay scales with display DPI.
- Microphone picker in the window, a `microphone` setting, and a `speakcraft devices` command that lists input names.
- Settings are saved to `config.toml` when dictation is enabled from the window. An optional checkbox stores the API key in Windows Credential Manager instead of the file.
- Optional paste insertion (`insertion = "paste"`). It marks dictated clipboard text to be skipped by clipboard history and cloud sync, and restores the previous clipboard text. It types instead when the clipboard holds anything other than plain text.
- One automatic transcription retry after HTTP 429, 500, 502, 503, or 504, or a failed connection. It honors `Retry-After` up to 5 seconds.
- Single-instance guard with a clear "already running" message.
- Windows end-to-end harness (`scripts/windows_e2e.py`) and a `test-audio` Cargo feature that replays a WAV file in place of the microphone, for testing only.
- Windows desktop window opened by double-clicking `speakcraft.exe`, with provider/model/language settings, optional cleanup, enable/disable controls, and visible status/errors.
- Masked session-only API-key entry without storing keys on disk or changing the process environment.
- Automatic loading of `config.toml` beside the executable for desktop launch; explicit `--config` overrides it. CLI configuration loading is unchanged.
- Desktop interaction tests for setup errors and service restart, plus session-credential and CLI compatibility contract tests.
- Usage guide covering setup, shortcuts, cancellation, recovery, privacy, and troubleshooting.
- Complete configuration reference with built-in defaults, local-provider examples, and required API contracts.
- Development guide with native/cross-build instructions, automated checks, and a pending Windows verification checklist.

### Changed

- The default transcription model is now `gpt-4o-mini-transcribe` (previously `whisper-1`). In a live comparison it resolved a spoken self-correction that `whisper-1` turned into an invented date after cleanup. Set `transcription_model = "whisper-1"` to keep the old model.
- Recordings above 16 kHz are downsampled to 16 kHz before upload, which cuts the upload to a third for 48 kHz microphones.
- AI cleanup receives the dictionary terms so their spelling survives editing.
- Canceling, disabling, or closing during processing now also skips any pending retry and the optional cleanup request.
- Without a Windows default input device, recording uses the first available microphone instead of failing.
- Launching without a subcommand opens the desktop UI instead of printing help and exiting. CLI `run`, `transcribe`, `check`, help, and version remain available.
- Minimize keeps dictation enabled; disabling or closing releases shortcuts and discards pending insertion and retained text. SpeakCraft's own settings window is excluded from dictation targets.
- Renamed the app to SpeakCraft; the Rust package and executable are now `speakcraft` and `speakcraft.exe` on Windows. Existing configuration files, environment-variable names, and dictation shortcuts remain compatible. Update launch commands to the new executable path.
- Reorganized the README around Windows quick start, features, build instructions, and links to detailed documentation.
- Documented the difference between the original archive and the local SpeakCraft build, upgrade steps, file-transcription usage, and unchanged configuration behavior.
- Made the MVP status, API costs, cloud data flow, and unverified Windows desktop behavior explicit.
- Capture now requests 16 kHz mono when the microphone offers it, falling back to the device default sample rate otherwise. Fallback audio is downmixed to mono without local resampling; provider sample-rate support determines compatibility. Echo cancellation, noise suppression, and auto gain remain at the Windows defaults.

### Fixed

- Typed insertion into Windows 11 Notepad arrived as one character repeated, such as `plain ssss…`, because Notepad reads the most recent injected character when it processes each keystroke. SpeakCraft now types one character at a time, about 100 per second, and stops cleanly if focus changes or Esc is pressed mid-insertion.

On 2026-10-16, 12 contract tests and 2 Windows unit tests passed, along with strict Clippy with and without `test-audio`. The end-to-end harness passed 13 scenarios against the release executable: hold-to-talk, tap-to-toggle, both cancel paths, a provider error, focus change with recovery, held modifiers, second instance, Notepad typing, Notepad pasting, clipboard restore, the recording limit, and one live OpenAI dictation with cleanup. Recorded TTS audio replaced the microphone because the test machine had no connected input device, so live microphone capture remains unverified.

On 2026-10-15, the desktop build passed eight contract tests and a Windows desktop interaction test. Native default, enabled/cleanup, and missing-credential windows were rendered and inspected. Live microphone capture, hotkey-driven recording, text insertion into other apps, and speech-model accuracy remain unverified. No desktop archive, installer, or public release was published.

## 0.1.0 — 2026-10-14

Initial local MVP. Source and an unsigned Windows x64 ZIP were prepared; no release was published.

### Added

- Windows global toggle, cancellation, and retained-transcript recovery shortcuts.
- Default-microphone capture using CPAL/WASAPI, mono PCM WAV encoding in memory, duration/size limits, and rejection of short or effectively silent recordings.
- OpenAI-compatible transcription with optional language and dictionary hints.
- Opt-in chat-completions cleanup with original-transcript fallback on errors or empty output.
- Unicode text insertion without clipboard replacement, with best-effort focus checks and retained text for explicit retry.
- `run`, `transcribe`, and `check` commands; file transcription and configuration checks are available outside Windows.
- TOML configuration, environment-variable credentials, HTTPS enforcement except loopback, redirect rejection, and request timeouts.
- Six contract tests, Linux orb setup, locked dependencies, Windows GNU cross-build support, and MIT license.

### Verification

- Six automated tests passed on Linux, and an HTTP-fixture CLI smoke check verified Unicode stdout.
- Formatting and strict Clippy checks passed, including Windows-target code.
- A Windows x64 release executable cross-built successfully and was packaged with checksum information.
- No real Windows desktop test or live speech-model accuracy test was performed.

### Known limitations

- Console-only app with fixed shortcuts and default microphone; no tray UI, installer, push-to-talk, or bundled offline model.
- A configured speech service is required. Optional cleanup uses a separate request to the same provider.
- Canceling during processing suppresses insertion but does not retract requests or stop optional cleanup already scheduled on the worker.
- Focus protection is best-effort. Elevated or incompatible target apps can block or ignore input; successful event injection does not prove text was inserted.

See [usage and limitations](docs/usage.md) for the full operating constraints.
