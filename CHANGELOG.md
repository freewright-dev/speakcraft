# Changelog

User-visible changes to Saykey. Versions match `Cargo.toml`. Local build entries do not imply a published release, signed installer, or completed Windows desktop verification.

## Unreleased

### Added

- Usage guide covering setup, shortcuts, cancellation, recovery, privacy, and troubleshooting.
- Complete configuration reference with built-in defaults, local-provider examples, and required API contracts.
- Development guide with native/cross-build instructions, automated checks, and a pending Windows verification checklist.

### Changed

- Renamed the app to Saykey; the Rust package and executable are now `saykey` and `saykey.exe` on Windows.
- Reorganized the README around Windows quick start, features, build instructions, and links to detailed documentation.
- Made the MVP status, API costs, cloud data flow, and unverified Windows behavior explicit.
- Capture now requests 16 kHz mono when the microphone offers it, falling back to the device default sample rate otherwise. Speech recognition resamples to 16 kHz regardless, so this trims upload size without changing behavior on drivers that expose only their mix format. Echo cancellation, noise suppression, and auto gain remain at the Windows defaults.

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
