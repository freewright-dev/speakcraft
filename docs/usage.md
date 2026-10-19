# Usage, privacy, and troubleshooting

[README](../README.md) · [Configuration](configuration.md) · [Development](development.md) · [Changelog](../CHANGELOG.md)

## Install and start

The desktop app targets Windows 10/11 x64. [Build from source](development.md#windows-build) to obtain the current `speakcraft.exe`, then double-click it. No terminal is needed. If you have a supplied ZIP containing the desktop build, extract it first; do not run the executable inside the ZIP. The app is unsigned and has no installer, auto-updater, or background service. No new desktop archive or public release has been published.

For an archive that includes `speakcraft.exe` and `SHA256SUMS.txt`, compare the executable's SHA-256 digest with its checksum entry:

```powershell
Get-FileHash .\speakcraft.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

A matching checksum detects corruption, not publisher authenticity: both files came from the same archive. Only run code from a source you trust. Alternatively, [build from source](development.md#windows-build).

Older archives use `rust-dictate.exe`; use that filename for their commands and checksum checks. A locally rebuilt `speakcraft.exe` will not match an older binary's checksum. When upgrading, close the old instance, keep your existing `config.toml`, and update any launch commands or shortcuts to the new executable path. The configuration keys, `OPENAI_API_KEY` variable, and dictation shortcuts are unchanged; no configuration migration is required.

Follow the [README quick start](../README.md#quick-start-windows-desktop) to configure the provider and enter your API key in the window. Click **Enable dictation** and wait for **Ready**. Missing credentials, invalid settings, or unavailable shortcuts appear in the window instead of closing it. Enabling dictation alone makes no provider request.

Desktop launch reads an explicit `--config` path, otherwise `config.toml` beside the executable, otherwise built-in defaults. A file-load error is shown in the window with default settings for review. Enabling dictation saves the window's settings to that same path (or beside the executable), without comments; a failed save is reported but does not block dictation. Entered keys are session-only unless **Remember this key on this PC** is checked, which stores the key in Windows Credential Manager for your account. Uncheck it and enable dictation again to remove the stored key. Disable dictation before editing settings. Keep secrets out of configuration files.

In Windows settings:

- **Windows 11:** Privacy & security → Microphone → allow microphone access and desktop app access.
- **Windows 10:** Privacy → Microphone → allow microphone access and desktop app access.
- **Both:** choose a microphone in SpeakCraft's **Microphone** list, or keep **Windows default** and select the input in Sound settings. If Windows reports no default input, SpeakCraft uses the first available microphone. Click **Refresh** after connecting a device.

Minimize the window to keep dictation enabled. Closing it exits the app, stops capture, and discards pending insertion and retained text. **Disable dictation** does the same without closing the window and releases the global shortcuts. An API request already sent may finish in the background, but its result is not inserted. Only one SpeakCraft (window or `run`) can be open at a time. No tray icon is installed.

## Dictate into an application

1. Enable dictation in SpeakCraft, then open a normal, non-administrator Notepad window and focus its editing area.
2. Hold **Ctrl+Alt+Space**. A rising tone plays and a red **Recording** pill appears above the taskbar.
3. Speak while holding the shortcut, then release it. A falling tone plays and the pill shows **Transcribing…**.
4. Wait for transcription and insertion. Do not move the caret, switch fields, or type while processing.
5. Review the inserted text, especially names, numbers, and any AI corrections.

For longer dictation, tap the shortcut instead: a press shorter than half a second keeps recording hands-free until you press it again. Turn off sounds or the status pill in the window, or with `sounds` and `overlay` in `config.toml`. The pill never takes focus or mouse clicks. Shortcuts are fixed in this version. Ctrl+Alt combinations may overlap with AltGr on some keyboard layouts.

Recording stops and proceeds to transcription automatically at the configured duration (120 seconds by default), or when its audio buffer reaches the size cap just below 24 MB. Recordings under 200 ms and effectively silent input are rejected. A microphone error discards the recording.

### Cancel or recover text

| State | Ctrl+Alt+Esc | Ctrl+Alt+V |
| --- | --- | --- |
| Recording | Discard audio without submitting it; clear retained text | No action |
| Processing | Suppress the result; clear retained text; wait for the request to finish | No action |
| Idle | Clear retained text | Insert retained text into the currently focused field |

Canceling during processing does **not** abort or retract the API request already in flight, but it prevents any retry and the optional cleanup request. Another recording cannot start until that request finishes. Closing the app cannot retract a request already received by the provider either.

If a focus change is detected or insertion fails, SpeakCraft reports that text was retained. Focus the intended field, press **Ctrl+Alt+V**, and release the keys. If partial text appeared earlier, remove it before retrying to avoid duplication. Only one transcript can be retained; it is not a history feature. Use cancellation to clear it explicitly. SpeakCraft does not record for or insert text into its own settings window.

## Command reference

| Command | Behavior |
| --- | --- |
| `speakcraft` | Open the Windows desktop window; also works by double-clicking the executable |
| `speakcraft --config config.toml` | Open the desktop window using an explicit configuration file |
| `speakcraft --config config.toml run` | Console-mode Windows live dictation; reads environment credentials and reserves shortcuts at startup |
| `speakcraft --config config.toml transcribe recording.wav` | Upload an existing file and print the transcript to stdout |
| `speakcraft --config config.toml check` | Validate configuration without contacting a provider or opening the microphone |
| `speakcraft devices` | List microphone names for the `microphone` setting (Windows) |
| `speakcraft --help` | Show commands and options |
| `speakcraft --version` | Show the package version |

Use `.\speakcraft.exe` as the command in Windows PowerShell, or `.\target\release\speakcraft.exe` when running from the source root. Explicit relative paths resolve against the current working directory. `--config` can appear before or after a subcommand. For `run`, `transcribe`, and `check`, omitting it uses built-in defaults; only desktop launch automatically checks beside the executable.

CLI commands still use environment credentials. To enter a key without putting it in shell history, use the same PowerShell session to set it and launch the command:

```powershell
$secret = Read-Host "OpenAI API key" -AsSecureString
$env:OPENAI_API_KEY = [System.Net.NetworkCredential]::new("", $secret).Password
.\speakcraft.exe --config config.toml run
```

This does not persist the key to future sessions. A desktop app launched separately from Explorer does not inherit a variable set only in this PowerShell session; use its API-key field instead.

`transcribe` accepts nonempty files up to 24,000,000 bytes. Formats depend on the provider; the app does not convert existing files. It uses the same optional cleanup setting as live dictation, but never inserts text into another application. Warnings go to stderr; the transcript goes to stdout. Redirect stdout only if you intend to save that text.

For example, transcribe an existing file after configuring provider credentials:

```powershell
.\speakcraft.exe --config config.toml transcribe .\recording.wav
```

This command uploads the file to the configured provider and may incur charges; it is not an offline validation command.

`check` does not verify the key, provider, microphone, shortcuts, or application compatibility. Startup/command failures exit nonzero. Live dictation logs individual recording or provider errors and remains available for another attempt.

## Privacy and limitations

- **Network:** audio goes to the configured speech endpoint after recording stops, including automatic stops. When cleanup is enabled, the transcript goes to the same base URL's chat-completions endpoint. Provider charges and retention policies apply; this app does not control them.
- **Local data:** live recordings and retained transcripts stay in process memory. The app writes no recording files, transcript history, analytics, or screen captures. Enabling dictation from the window writes non-secret settings to `config.toml`. Dictation does not read application content. With the default typing insertion it never touches the clipboard; see **Insertion** for paste mode. Ordinary memory deallocation is not secure erasure; Windows paging and crash dumps are outside its control.
- **Credentials:** the desktop API-key field is masked and kept in process memory. It is stored only if you check **Remember this key on this PC**, and then only in Windows Credential Manager for your Windows account on this PC, never in `config.toml`. Any process running as your account can read that entry. A typed key overrides the configured environment variable; leaving the field blank uses that variable. Unchecking **Provider requires an API key** omits authentication. CLI commands read environment credentials. Do not store keys in TOML, paste them into issue reports, or share environment dumps. Network requests require HTTPS except for explicit loopback addresses, and redirects are rejected.
- **Focus:** protection is best-effort, not a security boundary. Checks use foreground-window and focused-control handles. Browser fields can share a handle; moving the caret and very brief focus changes may go undetected. Do not dictate into password fields or terminals.
- **Insertion:** by default the app types Unicode input events one character at a time, about 100 per second. It does not modify the clipboard or synthesize an Enter shortcut. Typing stops if focus changes or Esc is pressed; the rest is retained for Ctrl+Alt+V, so remove the partial text first. With `insertion = "paste"` (**Pasting** in the window), SpeakCraft puts the transcript on the clipboard, marked to be skipped by Windows clipboard history and cloud sync, then sends Ctrl+V and restores the previous clipboard text after 400 ms. A target that reads the clipboard later than that may paste the restored text instead. Paste mode types instead when the clipboard holds anything other than plain text, such as images, files, or rich text, because those cannot be restored. Multiline text and Unicode support depend on the receiving app. Windows accepting the events does not prove the target inserted them. Insertion is capped at 16,000 UTF-16 code units; longer transcripts remain retained but cannot be inserted through the recovery shortcut.
- **AI output:** transcription and cleanup can be wrong. Cleanup instructions ask the model to preserve meaning and treat dictated instructions as text, but do not guarantee either. Check the output before sending or acting on it.
- **Missing features:** no tray icon, configurable shortcuts, streaming transcription, snippets, app-specific styles, automatic dictionary learning, account sync, offline model, or installer. A compatible local provider must be installed separately.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Console disappears after double-clicking | You may be running the old console-only build. Rebuild and open `target\release\speakcraft.exe`; the current executable opens a desktop window without arguments. |
| `Set the OPENAI_API_KEY environment variable` | Paste the key into the desktop API-key field, or set the variable in the same terminal used to launch a CLI command. Check `api_key_env` if customized. |
| Window settings cannot be edited | Click **Disable dictation** first, then edit and re-enable. Changes are not saved between app launches. |
| Hotkeys do nothing while SpeakCraft is focused | Focus a text field in another application; SpeakCraft excludes its own settings fields from dictation. |
| Configuration error | Check spelling and value types. Unknown keys are rejected. Use `check` before `run`. |
| `SpeakCraft is already running` | Use the open SpeakCraft window, or close it (or the console `run` session) first. |
| `No microphone found` or `Microphone "…" is not connected` | Connect a microphone, click **Refresh**, and choose it or **Windows default**. `speakcraft devices` lists the names Windows offers; an empty list means Windows reports no active input device. |
| No default microphone or access denied | Check desktop microphone permissions, device connection, and other apps holding the device. |
| Unsupported microphone sample format | The capture path accepts float32, signed 16-bit, or unsigned 16-bit samples. Try another default device or its supported Windows format settings. |
| Too short or no audible input | Speak for more than 200 ms; check mute, input level, and selected default microphone. |
| Cannot register a shortcut | Close another SpeakCraft instance or another app using Ctrl+Alt+Space, Ctrl+Alt+Esc, or Ctrl+Alt+V. |
| HTTP 401/403 | Check provider credentials, API access, and model permissions. |
| HTTP 404 | Check the API base URL and configured model. The app appends `/audio/transcriptions` and `/chat/completions`. |
| HTTP 429 or 5xx | SpeakCraft already retried once. Check quota or provider availability and record again later. |
| Repeated characters in Notepad (`ssss…`) | Builds before 2026-10-16 typed the whole transcript in one batch. Update SpeakCraft, or set `insertion = "paste"`. |
| Cleanup failed | The original transcript is used. Confirm that the provider supports chat completions and the configured cleanup model. |
| Text retained or shortcut-release warning | Focus the intended field, press Ctrl+Alt+V, and release all shortcut/modifier keys promptly. |
| Recording stops when releasing the shortcut | That is hold-to-talk. Tap the shortcut (shorter than half a second) for hands-free recording. |
| No text or partial text despite insertion | Try non-elevated Notepad. Elevated apps, secure desktops, games, and some editors may reject Unicode input. Do not elevate the dictation app as a general workaround. |
| Still processing after cancel | Cancellation suppresses insertion, retries, and cleanup, not the request already in flight. Each API request has a 90-second timeout. |

When reporting a problem, include Windows version, target app, microphone type, the non-sensitive error message, and whether cleanup was enabled. Exclude API keys, private audio, and transcripts unless you deliberately choose to share them.
