use saykey::{audio::encode_wav, config::Config, pipeline::Pipeline};
use std::{io::Cursor, thread, time::Duration};
use tiny_http::{Header, Response, Server};

type Requests = Vec<(String, Vec<u8>)>;

// A local HTTP fixture: tests exercise reqwest serialization and response handling,
// not a substituted Rust client. No paid service, credentials, or microphone used.
fn provider(replies: Vec<(u16, &'static str)>) -> (Config, thread::JoinHandle<Requests>) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let config = Config {
        base_url: format!("http://{}/v1", server.server_addr()),
        api_key_env: String::new(),
        ..Config::default()
    };
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for (status, body) in replies {
            let mut request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .expect("API request");
            let mut bytes = vec![];
            request.as_reader().read_to_end(&mut bytes).unwrap();
            requests.push((request.url().to_owned(), bytes));
            request
                .respond(
                    Response::from_string(body)
                        .with_status_code(status)
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        )
                        .with_header(Header::from_bytes("Location", "/redirect-trap").unwrap()),
                )
                .unwrap();
        }
        // Keep the endpoint alive so following a redirect cannot masquerade as
        // correct rejection just because the fixture closed its socket.
        assert!(
            server
                .recv_timeout(Duration::from_millis(100))
                .unwrap()
                .is_none(),
            "Unexpected request: redirect followed or disabled cleanup executed"
        );
        requests
    });
    (config, worker)
}

fn wav() -> Vec<u8> {
    encode_wav(&[-1200, 2345, -32768, 32767].repeat(800), 16_000).unwrap()
}

#[test]
fn wav_preserves_pcm_and_rejects_silence_or_short_recordings() {
    let samples = [-1200, 2345, -32768, 32767].repeat(800);
    let bytes = encode_wav(&samples, 16_000).unwrap();
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.spec().sample_rate, 16_000);
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().bits_per_sample, 16);
    assert_eq!(
        reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        samples
    );
    assert!(encode_wav(&samples[..3199], 16_000).is_err());
    assert!(encode_wav(&vec![0; 3200], 16_000).is_err());
    assert!(encode_wav(&samples, 0).is_err());
}

#[test]
fn configuration_rejects_insecure_endpoints_typos_and_invalid_limits() {
    Config::default().validate().unwrap();
    let example: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
    example.validate().unwrap();
    assert!(toml::from_str::<Config>("cleenup = true").is_err());
    for url in [
        "http://example.com/v1",
        "https://name:password@example.com/v1",
        "https://example.com/v1?key=test",
        "ftp://127.0.0.1",
    ] {
        assert!(
            Config {
                base_url: url.into(),
                ..Config::default()
            }
            .validate()
            .is_err(),
            "{url}"
        );
    }
    for url in [
        "http://127.0.0.1:8080/v1",
        "http://[::1]:8080/v1",
        "https://example.com/v1",
    ] {
        Config {
            base_url: url.into(),
            ..Config::default()
        }
        .validate()
        .unwrap();
    }
    for seconds in [0, 121] {
        assert!(
            Config {
                max_recording_seconds: seconds,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
    }
    for seconds in [1, 120] {
        Config {
            max_recording_seconds: seconds,
            ..Config::default()
        }
        .validate()
        .unwrap();
    }
}

#[test]
fn transcription_uploads_audio_language_and_dictionary_without_cleanup() {
    let (mut config, server) = provider(vec![(200, r#"{"text":"  Grüezi Zürich 👋  "}"#)]);
    config.language = Some("de".into());
    config.dictionary = vec!["Zürich".into(), "CPAL".into()];
    let audio = wav();
    let result = Pipeline::new(config)
        .unwrap()
        .transcribe(audio.clone(), "voice.wav")
        .unwrap();
    assert_eq!(result.text, "Grüezi Zürich 👋");
    assert!(!result.cleanup_failed);
    let requests = server.join().unwrap();
    assert_eq!(requests[0].0, "/v1/audio/transcriptions");
    let body = String::from_utf8_lossy(&requests[0].1);
    for field in [
        "name=\"model\"\r\n\r\nwhisper-1",
        "name=\"response_format\"\r\n\r\njson",
        "name=\"language\"\r\n\r\nde",
        "name=\"prompt\"\r\n\r\nZürich, CPAL",
        "filename=\"voice.wav\"",
    ] {
        assert!(body.contains(field), "Missing multipart field: {field}");
    }
    assert!(requests[0].1.windows(audio.len()).any(|part| part == audio));
}

#[test]
fn cleanup_returns_edited_text_and_sends_dictation_as_data() {
    let (mut config, server) = provider(vec![
        (200, r#"{"text":"uh meet at 3 no at 4 in Zürich"}"#),
        (
            200,
            r#"{"choices":[{"message":{"content":"Meet at 4 in Zürich."}}]}"#,
        ),
    ]);
    config.cleanup = true;
    let result = Pipeline::new(config)
        .unwrap()
        .transcribe(wav(), "voice.wav")
        .unwrap();
    assert_eq!(result.text, "Meet at 4 in Zürich.");
    assert!(!result.cleanup_failed);
    let requests = server.join().unwrap();
    assert_eq!(requests[1].0, "/v1/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(&requests[1].1).unwrap();
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(
        body["messages"][1]["content"],
        "uh meet at 3 no at 4 in Zürich"
    );
}

#[test]
fn unavailable_or_empty_cleanup_never_loses_the_original_transcript() {
    for reply in [
        (503, "unavailable"),
        (200, r#"{"choices":[]}"#),
        (200, r#"{"choices":[{"message":{"content":" "}}]}"#),
        (200, "not json"),
    ] {
        let (mut config, server) =
            provider(vec![(200, r#"{"text":"Original 17, not 71."}"#), reply]);
        config.cleanup = true;
        let result = Pipeline::new(config)
            .unwrap()
            .transcribe(wav(), "voice.wav")
            .unwrap();
        assert_eq!(result.text, "Original 17, not 71.");
        assert!(result.cleanup_failed);
        server.join().unwrap();
    }
}

#[test]
fn transcription_rejects_errors_redirects_and_empty_or_malformed_responses() {
    for reply in [
        (401, r#"{"text":"must not insert"}"#),
        (500, "failure"),
        (302, r#"{"text":"must not insert"}"#),
        (200, "invalid JSON"),
        (200, r#"{"text":"  "}"#),
    ] {
        let (config, server) = provider(vec![reply]);
        assert!(
            Pipeline::new(config)
                .unwrap()
                .transcribe(wav(), "voice.wav")
                .is_err()
        );
        server.join().unwrap();
    }
}
