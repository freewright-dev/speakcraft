use speakcraft::{
    audio::{downsample, encode_wav},
    config::{Config, Insertion},
    pipeline::Pipeline,
};
use std::{
    borrow::Cow,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
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
fn session_credentials_authenticate_without_an_environment_variable() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let config = Config {
        base_url: format!("http://{}/v1", server.server_addr()),
        api_key_env: "SPEAKCRAFT_TEST_MISSING_CREDENTIAL".into(),
        ..Config::default()
    };
    assert!(Pipeline::with_api_key(config.clone(), None).is_err());
    assert!(Pipeline::with_api_key(config.clone(), Some(" ".into())).is_err());
    let worker = thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(request.headers().iter().any(|header| {
            header.field.equiv("Authorization")
                && header.value.as_str() == "Bearer session-test-key"
        }));
        request
            .respond(Response::from_string(
                r#"{"text":"Session credential accepted."}"#,
            ))
            .unwrap();
    });
    let result = Pipeline::with_api_key(config, Some("session-test-key".into()))
        .unwrap()
        .transcribe(wav(), "voice.wav")
        .unwrap();
    assert_eq!(result.text, "Session credential accepted.");
    worker.join().unwrap();
}

#[test]
fn cli_commands_keep_output_and_exit_codes_with_the_desktop_executable() {
    let version = format!("speakcraft {}", env!("CARGO_PKG_VERSION"));
    for (args, expected) in [
        (vec!["--version"], version.as_str()),
        (vec!["--help"], "transcribe"),
        (
            vec!["--config", "config.example.toml", "check"],
            "Configuration valid.",
        ),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_speakcraft"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env_remove("OPENAI_API_KEY")
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(expected),
            "{:?}",
            output
        );
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_speakcraft"))
        .arg("not-a-command")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
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
        "name=\"model\"\r\n\r\ngpt-4o-mini-transcribe",
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
    config.dictionary = vec!["Zürich".into(), "SpeakCraft".into()];
    let result = Pipeline::new(config)
        .unwrap()
        .transcribe(wav(), "voice.wav")
        .unwrap();
    assert_eq!(result.text, "Meet at 4 in Zürich.");
    assert!(!result.cleanup_failed);
    let requests = server.join().unwrap();
    assert_eq!(requests[1].0, "/v1/chat/completions");
    let body: serde_json::Value = serde_json::from_slice(&requests[1].1).unwrap();
    assert!(body["messages"][0]["content"].as_str().unwrap().ends_with(
        "Spell these names and terms exactly as written when they occur: Zürich, SpeakCraft."
    ));
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
    for replies in [
        vec![(401, r#"{"text":"must not insert"}"#)],
        vec![(500, "failure"), (500, "failure")],
        vec![(302, r#"{"text":"must not insert"}"#)],
        vec![(200, "invalid JSON")],
        vec![(200, r#"{"text":"  "}"#)],
    ] {
        let (config, server) = provider(replies);
        assert!(
            Pipeline::new(config)
                .unwrap()
                .transcribe(wav(), "voice.wav")
                .is_err()
        );
        server.join().unwrap();
    }
}

#[test]
fn transient_failures_are_retried_once_and_client_errors_are_not() {
    let (config, server) = provider(vec![(503, "busy"), (200, r#"{"text":"Second attempt."}"#)]);
    let result = Pipeline::new(config)
        .unwrap()
        .transcribe(wav(), "voice.wav")
        .unwrap();
    assert_eq!(result.text, "Second attempt.");
    assert_eq!(server.join().unwrap().len(), 2);

    let (config, server) = provider(vec![(429, "slow down"), (502, "bad gateway")]);
    let error = Pipeline::new(config)
        .unwrap()
        .transcribe(wav(), "voice.wav")
        .err()
        .expect("two transient failures must fail");
    assert!(format!("{error:#}").contains("HTTP 502"), "{error:#}");
    assert_eq!(server.join().unwrap().len(), 2);

    // A retry would first wait a second, so a fast error proves there was none.
    let (config, server) = provider(vec![(400, "bad request")]);
    let started = Instant::now();
    assert!(
        Pipeline::new(config)
            .unwrap()
            .transcribe(wav(), "voice.wav")
            .is_err()
    );
    assert!(started.elapsed() < Duration::from_millis(900));
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn cancellation_stops_retries_and_cleanup_after_the_request_in_flight() {
    for (status, body) in [(503, "busy"), (200, r#"{"text":"Raw transcript."}"#)] {
        let server = Server::http("127.0.0.1:0").unwrap();
        let config = Config {
            base_url: format!("http://{}/v1", server.server_addr()),
            api_key_env: String::new(),
            cleanup: true,
            ..Config::default()
        };
        let canceled = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&canceled);
        let worker = thread::spawn(move || {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            // The user cancels while the provider is still working.
            cancel.store(true, Ordering::Release);
            request
                .respond(Response::from_string(body).with_status_code(status))
                .unwrap();
            assert!(
                server
                    .recv_timeout(Duration::from_millis(1500))
                    .unwrap()
                    .is_none(),
                "A retry or cleanup request was sent after cancellation"
            );
        });
        let result =
            Pipeline::new(config)
                .unwrap()
                .transcribe_cancelable(wav(), "voice.wav", &canceled);
        if status == 200 {
            assert_eq!(result.unwrap().text, "Raw transcript.");
        } else {
            assert!(result.is_err());
        }
        worker.join().unwrap();
    }
}

#[test]
fn downsampling_averages_to_16_khz_and_keeps_lower_rates() {
    let samples: Vec<i16> = (0..48_000).map(|i| [300, 600, 900][i % 3]).collect();
    let (output, rate) = downsample(&samples, 48_000);
    assert_eq!((rate, output.len()), (16_000, 16_000));
    assert!(output.iter().all(|&sample| sample == 600));
    let (output, rate) = downsample(&samples[..44_100], 44_100);
    assert_eq!((rate, output.len()), (16_000, 16_000));
    for rate in [16_000, 8_000] {
        let (output, kept) = downsample(&samples[..100], rate);
        assert!(matches!(output, Cow::Borrowed(_)));
        assert_eq!(kept, rate);
    }
    let (output, rate) = downsample(&samples[..48_000], 48_000);
    encode_wav(&output, rate).unwrap();
}

#[test]
fn settings_round_trip_through_a_saved_file_and_old_files_still_load() {
    let directory = std::env::temp_dir().join(format!("speakcraft-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    let config = Config {
        language: Some("de".into()),
        dictionary: vec!["Zürich".into()],
        cleanup: true,
        microphone: Some("USB Microphone".into()),
        sounds: false,
        overlay: false,
        insertion: Insertion::Paste,
        ..Config::default()
    };
    config.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("insertion = \"paste\""), "{text}");
    let loaded = Config::load(Some(&path)).unwrap();
    assert_eq!(loaded.language.as_deref(), Some("de"));
    assert_eq!(loaded.dictionary, ["Zürich"]);
    assert_eq!(loaded.microphone.as_deref(), Some("USB Microphone"));
    assert!(loaded.cleanup && !loaded.sounds && !loaded.overlay);
    assert_eq!(loaded.insertion, Insertion::Paste);
    assert!(
        Config {
            max_recording_seconds: 0,
            ..Config::default()
        }
        .save(&path)
        .is_err()
    );
    std::fs::remove_dir_all(&directory).unwrap();

    let old: Config =
        toml::from_str("transcription_model = \"whisper-1\"\ncleanup = true").unwrap();
    assert!(old.sounds && old.overlay && old.microphone.is_none());
    assert_eq!(old.insertion, Insertion::Type);
    assert!(toml::from_str::<Config>("insertion = \"clipboard\"").is_err());
    assert!(
        Config {
            microphone: Some(" ".into()),
            ..Config::default()
        }
        .validate()
        .is_err()
    );
}
