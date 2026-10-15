# Usage, privacy, and troubleshooting

[README](../README.md) · [Configuration](configuration.md) · [Development](development.md) · [Changelog](../CHANGELOG.md)

## Install and start

The live dictation target is Windows 10/11 x64. Extract the supplied Windows ZIP and open PowerShell in that folder. Do not run the executable directly inside the ZIP. The app is unsigned and has no installer, auto-updater, or background service.

Compare the executable's SHA-256 digest with the entry in the archive's `SHA256SUMS.txt`:

```powershell
Get-FileHash .\saykey.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

A matching checksum detects corruption, not publisher authenticity: both files came from the same archive. Only run code from a source you trust. Alternatively, [build from source](development.md#windows-build).

Follow the [README quick start](../README.md#quick-start-windows-archive) to copy the example configuration and enter your API key. Use the same PowerShell session to launch the app; the documented environment variable is not persisted to future sessions. If you built from source, substitute `.\target\release\saykey.exe` for `.\saykey.exe` in these examples.

In Windows settings:

- **Windows 11:** Privacy & security → Microphone → allow microphone access and desktop app access.
- **Windows 10:** Privacy → Microphone → allow microphone access and desktop app access.
- **Both:** select your desired default input device in Sound settings. The app has no device picker.

Keep the console open; minimizing is fine. Closing it exits the app and discards any retained transcript.

## Dictate into an application

1. Open a normal, non-administrator Notepad window and focus its editing area.
2. Press and release **Ctrl+Alt+Space**. The console reports `Recording…` once capture starts.
3. Speak. Press and release **Ctrl+Alt+Space** again when finished.
4. Wait for transcription and insertion. Do not move the caret, switch fields, or type while processing.
5. Review the inserted text, especially names, numbers, and any AI corrections.

This is toggle recording, not push-to-talk: holding a shortcut is unnecessary. Shortcuts are fixed in this version. Ctrl+Alt combinations may overlap with AltGr on some keyboard layouts.

Recording stops and proceeds to transcription automatically at the configured duration (120 seconds by default), or when its audio buffer reaches the size cap just below 24 MB. Recordings under 200 ms and effectively silent input are rejected. A microphone error discards the recording.

### Cancel or recover text

| State | Ctrl+Alt+Esc | Ctrl+Alt+V |
| --- | --- | --- |
| Recording | Discard audio without submitting it; clear retained text | No action |
| Processing | Suppress the result; clear retained text; wait for the request to finish | No action |
| Idle | Clear retained text | Insert retained text into the currently focused field |

Canceling during processing does **not** abort or retract API requests, and the worker may still perform optional cleanup. Another recording cannot start until that worker finishes. Closing the app cannot retract a request already received by the provider either.

If a focus change is detected or insertion fails, the console reports that text was retained. Focus the intended field, press **Ctrl+Alt+V**, and release the keys. If partial text appeared earlier, remove it before retrying to avoid duplication. Only one transcript can be retained; it is not a history feature. Use cancellation to clear it explicitly.

## Command reference

| Command | Behavior |
| --- | --- |
| `saykey --config config.toml run` | Windows live dictation; reads credentials and reserves shortcuts at startup |
| `saykey --config config.toml transcribe recording.wav` | Upload an existing file and print the transcript to stdout |
| `saykey --config config.toml check` | Validate configuration without contacting a provider or opening the microphone |
| `saykey --help` | Show commands and options |
| `saykey --version` | Show the package version |

Use `.\saykey.exe` as the command in Windows PowerShell. Paths are relative to the current working directory unless absolute. `--config` can appear before or after the subcommand. Omitting it uses built-in defaults; an adjacent `config.toml` is **not** loaded automatically.

`transcribe` accepts nonempty files up to 24,000,000 bytes. Formats depend on the provider; the app does not convert existing files. It uses the same optional cleanup setting as live dictation, but never inserts text into another application. Warnings go to stderr; the transcript goes to stdout. Redirect stdout only if you intend to save that text.

`check` does not verify the key, provider, microphone, shortcuts, or application compatibility. Startup/command failures exit nonzero. Live dictation logs individual recording or provider errors and remains available for another attempt.

## Privacy and limitations

- **Network:** audio goes to the configured speech endpoint after recording stops, including automatic stops. When cleanup is enabled, the transcript goes to the same base URL's chat-completions endpoint. Provider charges and retention policies apply; this app does not control them.
- **Local data:** live recordings and retained transcripts stay in process memory. The app writes no recording files, transcript history, analytics, or screen captures. It does not read application content or the clipboard. Ordinary memory deallocation is not secure erasure; Windows paging and crash dumps are outside its control.
- **Credentials:** keys are read from the configured environment variable. Do not store them in TOML, paste them into issue reports, or share environment dumps. Network requests require HTTPS except for explicit loopback addresses, and redirects are rejected.
- **Focus:** protection is best-effort, not a security boundary. Checks use foreground-window and focused-control handles. Browser fields can share a handle; moving the caret and very brief focus changes may go undetected. Do not dictate into password fields or terminals.
- **Insertion:** the app sends Unicode input events without modifying the clipboard or synthesizing an Enter shortcut. Multiline text and Unicode support depend on the receiving app. Windows accepting the events does not prove the target inserted them. Insertion is capped at 16,000 UTF-16 code units; longer transcripts remain retained but cannot be inserted through the recovery shortcut.
- **AI output:** transcription and cleanup can be wrong. Cleanup instructions ask the model to preserve meaning and treat dictated instructions as text, but do not guarantee either. Check the output before sending or acting on it.
- **Missing features:** no tray/overlay UI, configurable shortcuts, microphone picker, snippets, app-specific styles, automatic dictionary learning, account sync, offline model, or installer. A compatible local provider must be installed separately.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Console disappears after double-clicking | Open PowerShell and launch with the `run` subcommand so errors remain visible. |
| `Set the OPENAI_API_KEY environment variable` | Enter the key in the same PowerShell session used to launch the app; check `api_key_env` if customized. |
| Configuration error | Check spelling and value types. Unknown keys are rejected. Use `check` before `run`. |
| No default microphone or access denied | Check default input, desktop microphone permissions, device connection, and other apps holding the device. |
| Unsupported microphone sample format | The capture path accepts float32, signed 16-bit, or unsigned 16-bit samples. Try another default device or its supported Windows format settings. |
| Too short or no audible input | Speak for more than 200 ms; check mute, input level, and selected default microphone. |
| Cannot register a shortcut | Close another Saykey instance or another app using Ctrl+Alt+Space, Ctrl+Alt+Esc, or Ctrl+Alt+V. |
| HTTP 401/403 | Check provider credentials, API access, and model permissions. |
| HTTP 404 | Check the API base URL and configured model. The app appends `/audio/transcriptions` and `/chat/completions`. |
| HTTP 429 or 5xx | Check quota or provider availability and retry later. The app does not retry failed dictations automatically. |
| Cleanup failed | The original transcript is used. Confirm that the provider supports chat completions and the configured cleanup model. |
| Text retained or shortcut-release warning | Focus the intended field, press Ctrl+Alt+V, and release all shortcut/modifier keys promptly. |
| No text or partial text despite insertion | Try non-elevated Notepad. Elevated apps, secure desktops, games, and some editors may reject Unicode input. Do not elevate the dictation app as a general workaround. |
| Still processing after cancel | Cancellation suppresses insertion, not the network operation. Each API request has a 90-second timeout; optional cleanup is a separate request. |

When reporting a problem, include Windows version, target app, microphone type, the non-sensitive error message, and whether cleanup was enabled. Exclude API keys, private audio, and transcripts unless you deliberately choose to share them.
