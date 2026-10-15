# Usage, privacy, and troubleshooting

[README](../README.md) · [Configuration](configuration.md) · [Development](development.md) · [Changelog](../CHANGELOG.md)

## Install and start

The desktop app targets Windows 10/11 x64. [Build from source](development.md#windows-build) to obtain the current `saykey.exe`, then double-click it. No terminal is needed. If you have a supplied ZIP containing the desktop build, extract it first; do not run the executable inside the ZIP. The app is unsigned and has no installer, auto-updater, or background service. No new desktop archive or public release has been published.

For an archive that includes `saykey.exe` and `SHA256SUMS.txt`, compare the executable's SHA-256 digest with its checksum entry:

```powershell
Get-FileHash .\saykey.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

A matching checksum detects corruption, not publisher authenticity: both files came from the same archive. Only run code from a source you trust. Alternatively, [build from source](development.md#windows-build).

Older archives use `rust-dictate.exe`; use that filename for their commands and checksum checks. A locally rebuilt `saykey.exe` will not match an older binary's checksum. When upgrading, close the old instance, keep your existing `config.toml`, and update any launch commands or shortcuts to the new executable path. The configuration keys, `OPENAI_API_KEY` variable, and dictation shortcuts are unchanged; no configuration migration is required.

Follow the [README quick start](../README.md#quick-start-windows-desktop) to configure the provider and enter your API key in the window. Click **Enable dictation** and wait for **Ready**. Missing credentials, invalid settings, or unavailable shortcuts appear in the window instead of closing it. Enabling dictation alone makes no provider request.

Desktop launch reads an explicit `--config` path, otherwise `config.toml` beside the executable, otherwise built-in defaults. A file-load error is shown in the window with default settings for review. Window edits and entered keys are session-only. Disable dictation before editing settings. Keep secrets out of configuration files.

In Windows settings:

- **Windows 11:** Privacy & security → Microphone → allow microphone access and desktop app access.
- **Windows 10:** Privacy → Microphone → allow microphone access and desktop app access.
- **Both:** select your desired default input device in Sound settings. The app has no device picker.

Minimize the window to keep dictation enabled. Closing it exits the app, stops capture, and discards pending insertion and retained text. **Disable dictation** does the same without closing the window and releases the global shortcuts. An API request already sent may finish in the background, but its result is not inserted. No tray icon is installed.

## Dictate into an application

1. Enable dictation in Saykey, then open a normal, non-administrator Notepad window and focus its editing area.
2. Press and release **Ctrl+Alt+Space**. Saykey reports `Recording…` once capture starts.
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

If a focus change is detected or insertion fails, Saykey reports that text was retained. Focus the intended field, press **Ctrl+Alt+V**, and release the keys. If partial text appeared earlier, remove it before retrying to avoid duplication. Only one transcript can be retained; it is not a history feature. Use cancellation to clear it explicitly. Saykey does not record for or insert text into its own settings window.

## Command reference

| Command | Behavior |
| --- | --- |
| `saykey` | Open the Windows desktop window; also works by double-clicking the executable |
| `saykey --config config.toml` | Open the desktop window using an explicit configuration file |
| `saykey --config config.toml run` | Console-mode Windows live dictation; reads environment credentials and reserves shortcuts at startup |
| `saykey --config config.toml transcribe recording.wav` | Upload an existing file and print the transcript to stdout |
| `saykey --config config.toml check` | Validate configuration without contacting a provider or opening the microphone |
| `saykey --help` | Show commands and options |
| `saykey --version` | Show the package version |

Use `.\saykey.exe` as the command in Windows PowerShell, or `.\target\release\saykey.exe` when running from the source root. Explicit relative paths resolve against the current working directory. `--config` can appear before or after a subcommand. For `run`, `transcribe`, and `check`, omitting it uses built-in defaults; only desktop launch automatically checks beside the executable.

CLI commands still use environment credentials. To enter a key without putting it in shell history, use the same PowerShell session to set it and launch the command:

```powershell
$secret = Read-Host "OpenAI API key" -AsSecureString
$env:OPENAI_API_KEY = [System.Net.NetworkCredential]::new("", $secret).Password
.\saykey.exe --config config.toml run
```

This does not persist the key to future sessions. A desktop app launched separately from Explorer does not inherit a variable set only in this PowerShell session; use its API-key field instead.

`transcribe` accepts nonempty files up to 24,000,000 bytes. Formats depend on the provider; the app does not convert existing files. It uses the same optional cleanup setting as live dictation, but never inserts text into another application. Warnings go to stderr; the transcript goes to stdout. Redirect stdout only if you intend to save that text.

For example, transcribe an existing file after configuring provider credentials:

```powershell
.\saykey.exe --config config.toml transcribe .\recording.wav
```

This command uploads the file to the configured provider and may incur charges; it is not an offline validation command.

`check` does not verify the key, provider, microphone, shortcuts, or application compatibility. Startup/command failures exit nonzero. Live dictation logs individual recording or provider errors and remains available for another attempt.

## Privacy and limitations

- **Network:** audio goes to the configured speech endpoint after recording stops, including automatic stops. When cleanup is enabled, the transcript goes to the same base URL's chat-completions endpoint. Provider charges and retention policies apply; this app does not control them.
- **Local data:** live recordings and retained transcripts stay in process memory. The app writes no recording files, transcript history, analytics, or screen captures. Dictation does not read application content or the clipboard; the settings window supports normal clipboard paste when you request it. Ordinary memory deallocation is not secure erasure; Windows paging and crash dumps are outside its control.
- **Credentials:** the desktop API-key field is masked and kept in process memory, not saved to disk. A typed key overrides the configured environment variable for that session; leaving the field blank uses that variable. Unchecking **Provider requires an API key** omits authentication. CLI commands read environment credentials. Do not store keys in TOML, paste them into issue reports, or share environment dumps. Network requests require HTTPS except for explicit loopback addresses, and redirects are rejected.
- **Focus:** protection is best-effort, not a security boundary. Checks use foreground-window and focused-control handles. Browser fields can share a handle; moving the caret and very brief focus changes may go undetected. Do not dictate into password fields or terminals.
- **Insertion:** the app sends Unicode input events without modifying the clipboard or synthesizing an Enter shortcut. Multiline text and Unicode support depend on the receiving app. Windows accepting the events does not prove the target inserted them. Insertion is capped at 16,000 UTF-16 code units; longer transcripts remain retained but cannot be inserted through the recovery shortcut.
- **AI output:** transcription and cleanup can be wrong. Cleanup instructions ask the model to preserve meaning and treat dictated instructions as text, but do not guarantee either. Check the output before sending or acting on it.
- **Missing features:** no tray/overlay UI, configurable shortcuts, microphone picker, snippets, app-specific styles, automatic dictionary learning, account sync, offline model, or installer. A compatible local provider must be installed separately.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Console disappears after double-clicking | You may be running the old console-only build. Rebuild and open `target\release\saykey.exe`; the current executable opens a desktop window without arguments. |
| `Set the OPENAI_API_KEY environment variable` | Paste the key into the desktop API-key field, or set the variable in the same terminal used to launch a CLI command. Check `api_key_env` if customized. |
| Window settings cannot be edited | Click **Disable dictation** first, then edit and re-enable. Changes are not saved between app launches. |
| Hotkeys do nothing while Saykey is focused | Focus a text field in another application; Saykey excludes its own settings fields from dictation. |
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
