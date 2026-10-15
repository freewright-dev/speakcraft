# Saykey

**Speak, then insert text where you work.** Saykey is a Windows-first Rust console app: press a global shortcut to record, press it again to transcribe, and insert the result into the focused application. Optional AI cleanup asks the model to remove fillers and handle spoken corrections.

**Status: 0.1.0 MVP, not a production release.** The native Windows x64 release build, all six contract tests, formatting, strict Clippy, and CLI help/version/configuration checks pass. The initial MVP was also cross-built and tested on Linux. Real Windows microphone capture, shortcuts, text insertion, and speech recognition quality still require desktop testing.

This is an independent implementation inspired by Wispr Flow's public behavior, not its proprietary source, private APIs, branding, or model weights. It is not affiliated with Wispr Flow or feature-equivalent to it.

## Features

- Global start/stop, cancel, and transcript-recovery shortcuts on Windows.
- Default-microphone recording with in-memory mono WAV encoding.
- OpenAI-compatible transcription, language selection, and dictionary hints.
- Optional AI cleanup that falls back to the original transcript on errors.
- Unicode text insertion without reading or replacing your clipboard.
- Best-effort focus checks and explicit retry when insertion is skipped.
- Existing-audio-file transcription and configuration checks on Windows, Linux, and macOS.

No tray app, overlay, installer, push-to-talk mode, or offline model is included. See [limitations and privacy](docs/usage.md#privacy-and-limitations) before dictating sensitive content.

## Quick start: Windows archive

Use Windows 10/11 x64. If you have a supplied Windows ZIP, extract it and open PowerShell in the extracted folder. The executable is unsigned; [verify the build](docs/usage.md#install-and-start) before running it. No Rust installation or administrator privileges are required for a prebuilt executable.

The Saykey rename is currently a local source/build change, not a published release or a newly packaged archive. To use the renamed executable, [build from source](#build-from-source). An older archive may still contain `rust-dictate.exe`; the [usage guide](docs/usage.md#install-and-start) explains the naming change.

You need your own speech provider credentials. The default is OpenAI, which may charge for API usage. Audio is sent to that provider after recording stops; cleanup sends the transcript in a second request when enabled.

```powershell
Copy-Item config.example.toml config.toml
$secret = Read-Host "OpenAI API key" -AsSecureString
$env:OPENAI_API_KEY = [System.Net.NetworkCredential]::new("", $secret).Password
.\saykey.exe --config config.toml check
.\saykey.exe --config config.toml run
```

Copy the configuration only on first setup; keep an existing `config.toml` when upgrading. The key prompt avoids placing the key itself in shell history. It still exists in this process environment; do not share it or put it in the TOML file.

Allow desktop microphone access in Windows privacy settings and select the default input in Sound settings. Leave the console running, focus Notepad, press **Ctrl+Alt+Space**, speak, then press the shortcut again and release the keys. Keep the same text field focused until processing finishes.

| Shortcut | Action |
| --- | --- |
| Ctrl+Alt+Space | Start or finish recording |
| Ctrl+Alt+Esc | Cancel and clear retained text |
| Ctrl+Alt+V | Retry insertion of retained text into the current field |

Close the console to exit. Cancellation during processing suppresses insertion but cannot retract API requests already underway.

## Build from source

Install stable Rust through [rustup](https://rustup.rs/) and Visual Studio Build Tools with **Desktop development with C++**. In the source directory:

```powershell
cargo build --release --locked
.\target\release\saykey.exe --version
.\target\release\saykey.exe --config config.example.toml check
```

Run these commands from the directory containing `Cargo.toml`; the checkout folder does not need to be named `saykey`. The version command prints `saykey 0.1.0`, and `check` validates configuration without opening the microphone or contacting a provider.

Follow the quick start above, replacing `.\saykey.exe` with `.\target\release\saykey.exe`. Pass `--config config.toml` explicitly: an adjacent configuration file is not loaded automatically. See the [development guide](docs/development.md) for Linux cross-builds and tests.

## Documentation

- [Usage, privacy, and troubleshooting](docs/usage.md)
- [Configuration reference and local providers](docs/configuration.md)
- [Development, tests, and Windows verification checklist](docs/development.md)
- [Changelog](CHANGELOG.md)
- [Example configuration](config.example.toml)

## References and license

- [Wispr Flow features](https://wisprflow.ai/features): dictation across apps, cleanup, dictionary, and styles.
- [Wispr Flow setup guide](https://docs.wisprflow.ai/articles/3152211871-setup-guide): Windows permissions and shortcut-driven dictation.
- [OpenAI transcription API](https://platform.openai.com/docs/api-reference/audio/createTranscription): multipart request and JSON response contract.

These references describe public behavior, not Wispr Flow's internal architecture. Saykey is licensed under the [MIT License](LICENSE).
