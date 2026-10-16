# Development and verification

[README](../README.md) · [Usage](usage.md) · [Configuration](configuration.md) · [Changelog](../CHANGELOG.md)

Use the source archive or local repository, not just the prebuilt executable. Run commands from the directory containing `Cargo.toml`; its folder name does not affect the `saykey` package or executable name. Dependencies are recorded in `Cargo.lock`; keep `--locked` for repeatable dependency resolution. The initial MVP was built with Rust 1.99.0 on Linux. The native Windows checks below passed with Rust 1.97.1 (`x86_64-pc-windows-msvc`); a minimum supported Rust version has not been established.

## Windows build

Install stable Rust using [rustup](https://rustup.rs/) and Visual Studio Build Tools with the **Desktop development with C++** workload and Windows SDK. Use the normal x64 MSVC Rust toolchain for native development.

```powershell
cargo build --release --locked
.\target\release\saykey.exe --help
.\target\release\saykey.exe --version
.\target\release\saykey.exe --config config.example.toml check
```

These commands build and validate configuration without uploading audio. Double-click `target\release\saykey.exe` to open the desktop window without a console. The binary uses the Windows GUI subsystem and attaches to a parent console for CLI arguments. Follow the [usage guide](usage.md) to set provider credentials and test real dictation. The original Windows archive was cross-built with the GNU target on Linux; the new desktop build is verified natively on Windows.

`src/desktop.rs` owns the egui/eframe window and session settings. A worker thread runs the existing Windows hotkey loop and sends status/errors back to the window. Disabling or closing signals the worker to stop, release its shortcuts, discard capture/recovery state, and suppress pending insertion. Already-sent HTTP requests are not recalled. The settings window is excluded from dictation targets.

The package rename produces `target\release\saykey.exe`; Cargo may leave an old `rust-dictate.exe` in the build directory. Use the new path rather than assuming the old executable was updated. The local build does not create a ZIP, checksum manifest, installer, or published release.

## Linux development and Windows cross-build

Linux and macOS can run the file-transcription and configuration commands, but `run` deliberately fails outside Windows. Only Linux was used for the current non-Windows tests.

On a Debian-based Linux development machine, install stable Rust and the MinGW cross-linker:

```sh
sudo apt-get update
sudo apt-get install -y gcc-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo build --release --locked --target x86_64-pc-windows-gnu
```

The output is `target/x86_64-pc-windows-gnu/release/saykey.exe`. The source repository also includes an executable `.agents/setup` script for Amp's Linux orbs. It installs missing cross-build tools, prepares Rust components, and fetches locked dependencies. It is idempotent and does not start the app or configure provider credentials. It is stored locally with this project; it is not configured on a remote project's default branch.

## Automated checks

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

From Linux, also check Windows-only code:

```sh
cargo clippy --target x86_64-pc-windows-gnu --all-targets --locked -- -D warnings
cargo build --release --locked --target x86_64-pc-windows-gnu
```

The 12 integration tests in `tests/contracts.rs` cover several areas. Audio: in-memory PCM/WAV encoding, recording boundaries, and 16 kHz downsampling. Configuration: validation, and saving then reloading settings, including older files without the new fields. Provider calls: multipart upload fields, Unicode responses, optional cleanup with dictionary terms, and failure fallback. Errors: rejection of malformed responses and redirects, the single retry for transient failures, and that cancellation suppresses retries and cleanup. Also: session-key authentication and CLI output/exit codes. HTTP fixtures bind ephemeral loopback ports. They use no real microphone, credentials, paid API, or speech model.

On Windows, a unit test in `src/windows/credential.rs` saves, replaces, reads, and deletes a throwaway Credential Manager entry (`Saykey/test credential <pid>`); it never touches a real saved key. The egui accessibility test in `src/desktop.rs` clicks controls and types into fields to check missing-key errors, invalid language settings, enabling/disabling dictation, and re-enabling after shortcut release. Close other running Saykey dictation sessions before running it: the test reserves the real global shortcuts briefly, but does not record or submit audio. Default, enabled/cleanup, and missing-credential native windows were also rendered and inspected locally.

A passing API test confirms transport and response handling, **not** transcription accuracy, model adherence to cleanup instructions, or dictation in a Windows application. The existing Linux CLI smoke check also confirmed exact Unicode stdout through a local HTTP fixture. Cross-compilation only confirms that Windows code compiles and links.

## Windows end-to-end harness

`scripts/windows_e2e.py` (Python 3.10+, standard library only) drives a real `saykey.exe run` with synthesized global shortcuts. It uses a local mock provider that records every request, its own Tk test windows, and one Notepad file in a temporary directory. It types only into windows it creates, and it restores your clipboard's text afterwards. Do not use the keyboard while it runs. Close other Saykey instances first.

```powershell
# With a working microphone; --play makes it hear speech through the speakers.
cargo build --release --locked
python scripts\windows_e2e.py --play speech.wav

# Without a microphone: replay a 16-bit WAV in place of capture.
cargo build --release --locked --features test-audio --target-dir target\e2e
python scripts\windows_e2e.py --exe target\e2e\release\saykey.exe --audio speech.wav

# Add --live to also dictate once through the provider in OPENAI_API_KEY (paid).
```

Scenarios: tap-to-toggle with Unicode text, 16 kHz upload, and an unchanged clipboard; hold-to-talk, with the overlay visible without stealing focus and hidden after insertion; cancel while recording (no upload) and while processing; provider error; focus change followed by Ctrl+Alt+V recovery; insertion waiting for held modifiers; second instance refused; Notepad typing and Notepad pasting checked from the saved file; paste mode restoring the clipboard; the recording limit; optionally one live dictation.

The `test-audio` feature makes `SAYKEY_TEST_AUDIO=<file.wav>` replace the microphone. It exercises everything except device capture. Use a separate `--target-dir` as shown, and never ship a build that has it enabled. Generate test speech with Windows TTS, for example `System.Speech.Synthesis.SpeechSynthesizer.SetOutputToWaveFile` with a 16-bit mono format.

On 2026-10-16 all 13 scenarios passed with replayed audio and `--live`. The test machine had no active capture device, so live microphone capture is still unverified.

## Windows desktop verification checklist

Rows marked *automated* are covered by the end-to-end harness above. The rest are pending, not claims of completed verification. Use non-sensitive phrases and a disposable Notepad document. API calls may incur charges. Observe a local test provider's request log if you need to establish whether a cancellation uploaded audio.

| Case | Expected result |
| --- | --- |
| Tap Ctrl+Alt+Space to start and stop *(automated)* | Recording continues hands-free after the tap; stopping uploads audio and inserts the transcript after modifiers are released |
| Hold Ctrl+Alt+Space, then release *(automated)* | Release finishes and inserts; the overlay shows while recording without taking focus |
| Cancel while recording *(automated)* | Audio is discarded; provider receives no request for that recording |
| Cancel during a slow response *(automated)* | No text is inserted; no retry or cleanup follows; another recording waits for the existing request |
| Switch to another window during processing *(automated)* | Automatic insertion is skipped; Ctrl+Alt+V retries only after explicitly focusing the intended field |
| Hold shortcut keys after stopping *(automated)* | Text is not inserted while modifiers remain held; a prolonged hold retains text for retry |
| Accented text and emoji *(automated: Tk, Notepad)* | Unicode characters appear correctly in the receiving application |
| Keep a clipboard sentinel *(automated)* | Typing leaves the clipboard unchanged; paste mode restores its text |
| Real microphone capture | Speech from the selected or default microphone is transcribed; sounds play at start and stop |
| Disconnect or deny the microphone | An actionable capture error is reported and the app remains usable for another attempt |
| Launch a second instance *(automated for `run`)* | "Already running" is reported without disrupting the first instance |
| Reach the configured recording limit *(automated)* | Capture stops automatically and proceeds to transcription |
| Provider failure or invalid response *(automated: HTTP 500)* | No fabricated transcript is inserted; app allows another recording |
| Cleanup unavailable or empty | Original transcription is used with a warning |
| Elevated or unsupported target app | Inspect for blocked/partial input; remove partial text before any recovery attempt |
| Browsers, VS Code, Slack, terminals | Compare typing and paste modes per app |

Test each intended application separately. Browser control handles do not uniquely identify input fields; focus checking cannot make a wrong-field guarantee. Record the Windows build, device, provider/model, target application, and observed result without saving private speech or keys in the repository.

## Documenting future changes

Add user-visible changes under `Unreleased` in [CHANGELOG.md](../CHANGELOG.md). Use a dated version heading only when a version is actually prepared, and state whether it is local or published. Keep verification claims tied to checks that ran. Documentation edits do not require a package-version bump.
