# Configuration reference

[README](../README.md) · [Usage](usage.md) · [Development](development.md) · [Changelog](../CHANGELOG.md)

The desktop window edits provider URL, models, language, cleanup, microphone, sounds, overlay, and insertion. It starts with an explicit `--config` file, otherwise `config.toml` beside `saykey.exe`, otherwise built-in defaults. Disable dictation to edit settings, then re-enable. **Enabling dictation writes the window's settings back to that file** (or creates `config.toml` beside the executable). The saved file keeps every setting, including `dictionary` and `max_recording_seconds`, but drops comments, so keep annotated copies elsewhere.

To prepare settings by hand, copy [config.example.toml](../config.example.toml) to `config.toml` beside the executable. Keep any existing file when upgrading. To open the desktop with a different file or validate it from PowerShell:

```powershell
.\saykey.exe --config config.toml check
.\saykey.exe --config config.toml
```

Files are read once at startup; reopen the app after editing a file. Omitted fields use built-in defaults. Unknown fields and invalid TOML fail validation. The desktop displays file-load errors and shows defaults for review; CLI commands exit nonzero. `config.toml` and `.env` are Git-ignored, but **the app does not read `.env` files**. CLI subcommands (`run`, `transcribe`, `check`) still require `--config` to load a file.

Paste a key into the masked desktop API-key field. It is used for this session only unless **Remember this key on this PC** is checked; then it is stored in Windows Credential Manager (generic credential `Saykey/API key`) and loaded at the next launch. Keys are never written to the configuration file or the process environment. Leave the field blank to use `api_key_env`; uncheck **Provider requires an API key** for an unauthenticated local server. Environment variables must exist before launching the app. Dictionary hints and recording limits are configured through the file, not the window.

Existing configuration files from before the Saykey rename still work. Keep `config.toml` and your provider's environment-variable name unchanged; only the executable name in launch commands changes. Dictionary hints are ordinary text, so add `"Saykey"` if you want the provider to recognize the new app name.

## Fields and built-in defaults

| Field | Type | Default | Contract |
| --- | --- | --- | --- |
| `base_url` | String | `"https://api.openai.com/v1"` | API prefix, not the full transcription endpoint. HTTPS required except HTTP on `localhost`, `127.0.0.1`, or `[::1]`. No embedded credentials, query, or fragment. |
| `api_key_env` | String | `"OPENAI_API_KEY"` | Name of an environment variable containing a nonempty bearer token. `""` omits authentication. Never put the actual key here. |
| `transcription_model` | String | `"gpt-4o-mini-transcribe"` | Nonblank model name supported by your speech provider. Use `"gpt-4o-transcribe"` for higher accuracy at a higher price, or `"whisper-1"` for the previous default and most local servers. |
| `language` | Optional string | Omitted | Omit for provider auto-detection; otherwise two lowercase ASCII letters, such as `"de"` or `"en"`. The provider determines language support. |
| `dictionary` | Array of strings | `[]` | Spelling hints joined with commas and sent as the transcription `prompt`. With cleanup on, they are also listed in the cleanup instructions. Not exact replacements or automatic learning. |
| `cleanup` | Boolean | `false` | Enable a second API request that edits the transcript. |
| `cleanup_model` | String | `"gpt-4o-mini"` | Chat-completions model; must be nonblank when cleanup is enabled. |
| `max_recording_seconds` | Integer | `120` | Inclusive range 1–120; controls live capture only, not existing files or API timeout. |
| `microphone` | Optional string | Omitted | Input device name as shown by `saykey devices` or the window's list. Omit to use the Windows default, or the first available input when Windows has none. A named device that is not connected fails with an error instead of silently recording elsewhere. |
| `sounds` | Boolean | `true` | Short tones when recording starts, stops, is canceled, or fails. |
| `overlay` | Boolean | `true` | Always-on-top status pill above the taskbar on the target's monitor. It never takes focus or mouse input. |
| `insertion` | `"type"` or `"paste"` | `"type"` | `"type"` sends characters one at a time (about 100 per second) and never touches the clipboard. `"paste"` is instant: it uses the clipboard and restores its previous plain text, typing instead when the clipboard holds other content. See [insertion privacy](usage.md#privacy-and-limitations). |

The example file includes three dictionary hints; the **built-in** dictionary is empty. An empty language string is not automatic detection: omit `language` instead.

## German dictation with cleanup

```toml
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
transcription_model = "gpt-4o-mini-transcribe"
language = "de"
dictionary = ["Zürich", "Saykey", "CPAL"]
cleanup = true
cleanup_model = "gpt-4o-mini"
max_recording_seconds = 60
```

Supply the key in the desktop window or through the [PowerShell prompt for CLI commands](usage.md#command-reference). Cleanup costs an additional request and may alter meaning. An unsuccessful, malformed, or empty cleanup response falls back to the original transcript and reports a warning. No cleaned or raw transcript history is saved by live dictation.

## Local OpenAI-compatible provider

Run a compatible server separately, then point the app at its API prefix. For example, **if** your server listens on loopback port 8080 and exposes a model named `whisper-1`:

```toml
base_url = "http://127.0.0.1:8080/v1"
api_key_env = ""
transcription_model = "whisper-1"
cleanup = false
```

These are connection settings for a server on **your Windows computer**, not a running service provided by this project. Change the port and model to match your server. No speech model or server is bundled. If the server requires authentication, set `api_key_env` to the name of its token environment variable instead of leaving it empty. A server on another machine must use HTTPS.

Both APIs use the same `base_url` and credentials; separate speech and cleanup providers are not configurable. Keep cleanup disabled unless the server also implements the required chat-completions API. “OpenAI-compatible” alone does not guarantee support for every field below.

## Required provider API contract

| Operation | Request | Expected successful response |
| --- | --- | --- |
| Transcription | `POST {base_url}/audio/transcriptions`, multipart fields `file`, `model`, `response_format=json`; optional `language` and `prompt` | A 2xx JSON object containing a nonempty string `text` after trimming |
| Cleanup | `POST {base_url}/chat/completions`, JSON `model` and `messages` with editor instructions in the system message and dictated text in the user message | A 2xx JSON object whose first `choices` item has nonempty `message.content` after trimming |

Live audio is mono signed 16-bit PCM WAV at 16 kHz. Saykey captures at 16 kHz when the microphone supports it. Otherwise it captures at the device's default rate and downsamples to 16 kHz before upload by averaging each output sample's span of input samples. Device rates at or below 16 kHz are uploaded unchanged. Audio is downmixed to mono either way. Files are uploaded with their filename and `application/octet-stream` content type. No streaming response or streaming dictation is requested.

Saykey does not configure echo cancellation, noise suppression, or automatic gain control: those remain under Windows/device control. Existing files passed to `transcribe` are uploaded unchanged.

Connection timeout is 10 seconds; each request has a 90-second overall timeout. Redirects are not followed. A transcription that fails with HTTP 429, 500, 502, 503, or 504, or cannot connect, is retried once. The retry waits for the provider's `Retry-After` seconds (at most 5) or 1 second, unless the dictation was canceled meanwhile. Other transcription failures return an error immediately. Optional cleanup is not retried; its failures keep the raw transcript. These timeouts, the retry policy, the 24 MB upload cap, and shortcuts are fixed in code, not TOML settings.
