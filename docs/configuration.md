# Configuration reference

[README](../README.md) · [Usage](usage.md) · [Development](development.md) · [Changelog](../CHANGELOG.md)

Copy [config.example.toml](../config.example.toml) to `config.toml`, edit it, then pass it explicitly:

```powershell
.\saykey.exe --config config.toml check
.\saykey.exe --config config.toml run
```

Configuration is read once at startup; restart after changes. Omitted fields use built-in defaults. Unknown fields and invalid TOML fail validation rather than being ignored. `config.toml` and `.env` are Git-ignored, but **the app does not read `.env` files**.

Existing configuration files from before the Saykey rename still work. Keep `config.toml` and your provider's environment-variable name unchanged; only the executable name in launch commands changes. Dictionary hints are ordinary text, so add `"Saykey"` if you want the provider to recognize the new app name.

## Fields and built-in defaults

| Field | Type | Default | Contract |
| --- | --- | --- | --- |
| `base_url` | String | `"https://api.openai.com/v1"` | API prefix, not the full transcription endpoint. HTTPS required except HTTP on `localhost`, `127.0.0.1`, or `[::1]`. No embedded credentials, query, or fragment. |
| `api_key_env` | String | `"OPENAI_API_KEY"` | Name of an environment variable containing a nonempty bearer token. `""` omits authentication. Never put the actual key here. |
| `transcription_model` | String | `"whisper-1"` | Nonblank model name supported by your speech provider. |
| `language` | Optional string | Omitted | Omit for provider auto-detection; otherwise two lowercase ASCII letters, such as `"de"` or `"en"`. The provider determines language support. |
| `dictionary` | Array of strings | `[]` | Spelling hints joined with commas and sent as the transcription `prompt`. Not exact replacements or automatic learning. |
| `cleanup` | Boolean | `false` | Enable a second API request that edits the transcript. |
| `cleanup_model` | String | `"gpt-4o-mini"` | Chat-completions model; must be nonblank when cleanup is enabled. |
| `max_recording_seconds` | Integer | `120` | Inclusive range 1–120; controls live capture only, not existing files or API timeout. |

The example file includes three dictionary hints; the **built-in** dictionary is empty. An empty language string is not automatic detection: omit `language` instead.

## German dictation with cleanup

```toml
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
transcription_model = "whisper-1"
language = "de"
dictionary = ["Zürich", "Saykey", "CPAL"]
cleanup = true
cleanup_model = "gpt-4o-mini"
max_recording_seconds = 60
```

Supply the key through the [PowerShell prompt in the README](../README.md#quick-start-windows-archive). Cleanup costs an additional request and may alter meaning. An unsuccessful, malformed, or empty cleanup response falls back to the original transcript and reports a warning. No cleaned or raw transcript history is saved by live dictation.

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

Live audio is mono signed 16-bit PCM WAV, captured at 16 kHz when the microphone supports it and otherwise at the device's default sample rate (downmixed to mono either way). Files are uploaded with their filename and `application/octet-stream` content type. No streaming response or streaming dictation is requested.

Saykey does not resample fallback recordings before upload; the provider must accept their sample rate. It does not configure echo cancellation, noise suppression, or automatic gain control: those remain under Windows/device control. Existing files passed to `transcribe` are uploaded unchanged.

Connection timeout is 10 seconds; each request has a 90-second overall timeout. Redirects are not followed. Transcription failures return an error; optional cleanup failures retain the raw transcript. There is no application-level retry loop. These timeouts, the 24 MB upload cap, and shortcuts are fixed in code, not TOML settings.
