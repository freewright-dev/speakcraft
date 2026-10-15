# Saykey

**Speak, then insert text where you work.** Saykey is a Windows desktop dictation app built with Rust. Double-click `saykey.exe` to configure your provider and enable dictation. Press a global shortcut to record, press it again to transcribe, and insert the result into the focused application. Optional AI cleanup asks the model to remove fillers and handle spoken corrections.

**Status: 0.1.0 MVP with unreleased desktop UI.** The native Windows x64 build, eight contract tests, and desktop setup/start/stop tests pass. The default, enabled, and missing-credential windows were rendered and inspected. Real microphone capture, speech recognition quality, and insertion into other apps still require end-to-end testing. No new public release or installer has been published.

This is an independent implementation inspired by Wispr Flow's public behavior, not its proprietary source, private APIs, branding, or model weights. It is not affiliated with Wispr Flow or feature-equivalent to it.

## Features

- Desktop window with provider settings, a masked session-only API key, enable/disable controls, status, and errors.
- Global start/stop, cancel, and transcript-recovery shortcuts on Windows.
- Default-microphone recording with in-memory mono WAV encoding.
- OpenAI-compatible transcription, language selection, and dictionary hints.
- Optional AI cleanup that falls back to the original transcript on errors.
- Unicode text insertion without reading or replacing your clipboard.
- Best-effort focus checks and explicit retry when insertion is skipped.
- Existing-audio-file transcription and configuration checks on Windows, Linux, and macOS.

No tray app, overlay, installer, push-to-talk mode, or offline model is included. See [limitations and privacy](docs/usage.md#privacy-and-limitations) before dictating sensitive content.

## Quick start: Windows desktop

Use Windows 10/11 x64. [Build from source](#build-from-source) to get the current `target\release\saykey.exe`. If you have a ZIP containing this desktop build, extract it first. Older archives contain the console-only app. The executable is unsigned; use only a build you trust. No Rust installation or administrator privileges are required to run a prebuilt executable.

1. Double-click **`saykey.exe`**. No terminal or command-line arguments are needed.
2. Review the **Provider URL** and **Transcription model**. The defaults use OpenAI.
3. Paste your provider's key into **API key — this session only**. It is masked and is not saved to disk. A compatible local server without authentication can use the unchecked **Provider requires an API key** option.
4. Optionally set a language such as `de` or `en`, or enable AI cleanup.
5. Click **Enable dictation**, wait for **Ready**, then focus a text field in another app and use the shortcuts below.

You need your own speech provider credentials. Audio is sent to that provider after recording stops; cleanup sends the transcript in a second request when enabled. Provider charges and privacy policies apply. Enabling dictation registers shortcuts but does not start recording or verify your key with the provider.

Settings edited in the window apply to the current session. For persistent non-secret defaults, place a `config.toml` beside the executable; desktop launch reads it automatically. Keep existing configuration files when upgrading. See [configuration](docs/configuration.md) for file settings and environment-variable credentials.

Allow desktop microphone access in Windows privacy settings and select the default input in Sound settings. Focus Notepad, press **Ctrl+Alt+Space**, speak, then press the shortcut again and release the keys. Keep the same text field focused until processing finishes; returning to Saykey during processing counts as a focus change.

| Shortcut | Action |
| --- | --- |
| Ctrl+Alt+Space | Start or finish recording |
| Ctrl+Alt+Esc | Cancel and clear retained text |
| Ctrl+Alt+V | Retry insertion of retained text into the current field |

Minimize Saykey to keep dictating. **Disable dictation** releases the shortcuts and discards pending insertion and retained text; closing the window exits. There is no tray icon. Cancellation cannot retract API requests already underway.

## Build from source

Install stable Rust through [rustup](https://rustup.rs/) and Visual Studio Build Tools with **Desktop development with C++**. In the source directory:

```powershell
cargo build --release --locked
.\target\release\saykey.exe --version
.\target\release\saykey.exe --config config.example.toml check
```

Run these commands from the directory containing `Cargo.toml`; the checkout folder does not need to be named `saykey`. The version command prints `saykey 0.1.0`, and `check` validates configuration without opening the microphone or contacting a provider.

Double-click `target\release\saykey.exe` to open the desktop window. The existing `run`, `transcribe`, and `check` CLI commands remain available; unlike desktop launch, they require `--config config.toml` to load a file. See the [usage guide](docs/usage.md#command-reference) and [development guide](docs/development.md) for CLI commands, cross-builds, and tests.

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
