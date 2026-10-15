# Development and verification

[README](../README.md) · [Usage](usage.md) · [Configuration](configuration.md) · [Changelog](../CHANGELOG.md)

Use the source archive or local repository, not just the prebuilt executable. Run commands from the directory containing `Cargo.toml`. Dependencies are recorded in `Cargo.lock`; keep `--locked` for repeatable dependency resolution. The initial MVP was built with Rust 1.99.0; no older minimum toolchain version has been verified.

## Windows build

Install stable Rust using [rustup](https://rustup.rs/) and Visual Studio Build Tools with the **Desktop development with C++** workload and Windows SDK. Use the normal x64 MSVC Rust toolchain for native development.

```powershell
cargo build --release --locked
.\target\release\saykey.exe --help
.\target\release\saykey.exe --config config.example.toml check
```

These commands build and validate configuration without uploading audio. Follow the [usage guide](usage.md) to set provider credentials and test real dictation. The Saykey rename was verified with a native Windows MSVC release build, all six integration tests, formatting, strict Clippy, and CLI help/version/configuration checks. Real Windows desktop dictation remains untested; the original Windows archive was cross-built with the GNU target on Linux.

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

The six integration tests in `tests/contracts.rs` exercise in-memory PCM/WAV encoding, recording boundaries, configuration validation, multipart upload fields, Unicode responses, optional cleanup, failure fallback, and rejection of malformed responses and redirects. HTTP fixtures bind ephemeral loopback ports. They use no real microphone, credentials, paid API, or speech model.

A passing API test confirms transport and response handling, **not** transcription accuracy, model adherence to cleanup instructions, or dictation in a Windows application. The existing Linux CLI smoke check also confirmed exact Unicode stdout through a local HTTP fixture. Cross-compilation only confirms that Windows code compiles and links.

## Windows desktop verification checklist

This checklist is pending, not a claim of completed verification. Use non-sensitive phrases and a disposable Notepad document. API calls may incur charges. Observe a local test provider's request log if you need to establish whether a cancellation uploaded audio.

| Case | Expected result |
| --- | --- |
| Start and stop with Ctrl+Alt+Space | Recording starts; stopping uploads audio and inserts the transcript after modifiers are released |
| Cancel while recording | Audio is discarded; provider receives no request for that recording |
| Cancel during a slow response | No text is inserted; another recording waits for the existing worker to finish |
| Switch to another window during processing | Automatic insertion is skipped; Ctrl+Alt+V retries only after explicitly focusing the intended field |
| Hold shortcut keys after stopping | Text is not inserted while modifiers remain held; a prolonged hold retains text for retry |
| Accented text and emoji | Unicode characters appear correctly in the receiving application |
| Keep a clipboard sentinel | Clipboard contents remain unchanged after dictation and recovery |
| Disconnect or deny the microphone | An actionable capture error is reported and the app remains usable for another attempt |
| Launch a second instance | Shortcut registration fails without disrupting the first instance |
| Reach the configured recording limit | Capture stops automatically and proceeds to transcription |
| Provider failure or invalid response | No fabricated transcript is inserted; app allows another recording |
| Cleanup unavailable or empty | Original transcription is used with a warning |
| Elevated or unsupported target app | Inspect for blocked/partial input; remove partial text before any recovery attempt |

Test each intended application separately. Browser control handles do not uniquely identify input fields; focus checking cannot make a wrong-field guarantee. Record the Windows build, device, provider/model, target application, and observed result without saving private speech or keys in the repository.

## Documenting future changes

Add user-visible changes under `Unreleased` in [CHANGELOG.md](../CHANGELOG.md). Use a dated version heading only when a version is actually prepared, and state whether it is local or published. Keep verification claims tied to checks that ran. Documentation edits do not require a package-version bump.
