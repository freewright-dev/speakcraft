use crate::{
    config::{Config, Insertion},
    pipeline::Pipeline,
    windows,
};
use anyhow::{Context, Result};
use eframe::egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

pub fn run(config_path: Option<PathBuf>) -> Result<()> {
    let _instance = windows::SingleInstance::acquire()?;
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("config.toml")));
    // Settings are saved where they were loaded from, else beside the executable.
    let save_path = config_path.clone().or_else(|| beside_exe.clone());
    let path = config_path.or_else(|| beside_exe.filter(|path| path.exists()));
    let config = Config::load(path.as_deref());
    let saved_key = windows::credential::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SpeakCraft")
            .with_inner_size([620.0, 820.0])
            .with_min_inner_size([440.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SpeakCraft",
        options,
        Box::new(move |cc| {
            Ok(Box::new(Desktop::new(
                config,
                path,
                save_path,
                saved_key,
                &cc.egui_ctx,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("Cannot open SpeakCraft desktop window: {error}"))
}

enum Event {
    Status(String),
    Finished(Result<(), String>),
}

struct Worker {
    stop: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
    thread: thread::JoinHandle<()>,
}

struct Desktop {
    config: Config,
    source: String,
    save_path: Option<PathBuf>,
    language: String,
    api_key: String,
    authenticated: bool,
    /// Store the typed key in Windows Credential Manager when enabling.
    remember_key: bool,
    key_saved: bool,
    microphones: Vec<String>,
    worker: Option<Worker>,
    ready: bool,
    status: String,
    error: Option<String>,
}

impl Desktop {
    fn new(
        config: Result<Config>,
        path: Option<PathBuf>,
        save_path: Option<PathBuf>,
        saved_key: Result<Option<String>>,
        ctx: &egui::Context,
    ) -> Self {
        ctx.set_visuals(egui::Visuals::light());
        ctx.style_mut(|style| {
            style.visuals.disabled_alpha = 0.75;
            style.spacing.item_spacing = egui::vec2(10.0, 9.0);
            style.spacing.button_padding = egui::vec2(14.0, 9.0);
            style.spacing.interact_size.y = 30.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        });
        let (config, mut error) = match config {
            Ok(config) => (config, None),
            Err(error) => (
                Config::default(),
                Some(format!(
                    "Could not load configuration: {error:#}. Defaults are shown; review the settings before enabling dictation."
                )),
            ),
        };
        let saved_key = saved_key.unwrap_or_else(|key_error| {
            error.get_or_insert_with(|| format!("{key_error:#}"));
            None
        });
        let loaded = path.map_or_else(
            || "Using built-in defaults.".to_owned(),
            |path| format!("Loaded {}.", path.display()),
        );
        Self {
            source: match &save_path {
                Some(save) => format!(
                    "{loaded} Settings are saved to {} when you enable dictation.",
                    save.display()
                ),
                None => format!("{loaded} Settings apply to this session only."),
            },
            save_path,
            language: config.language.clone().unwrap_or_default(),
            authenticated: !config.api_key_env.is_empty(),
            config,
            remember_key: saved_key.is_some(),
            key_saved: saved_key.is_some(),
            api_key: saved_key.unwrap_or_default(),
            microphones: windows::microphones(),
            worker: None,
            ready: false,
            status:
                "Set up your provider, then enable dictation. No recording starts automatically."
                    .into(),
            error,
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let mut config = self.config.clone();
        config.language = if self.language.trim().is_empty() {
            None
        } else {
            Some(self.language.trim().to_owned())
        };
        if self.authenticated {
            if config.api_key_env.is_empty() {
                config.api_key_env = "OPENAI_API_KEY".into();
            }
        } else {
            config.api_key_env.clear();
        }
        if let Err(error) = config.validate() {
            self.error = Some(format!("{error:#}"));
            return;
        }
        // A typed key overrides the environment for this worker only.
        let key = if self.authenticated && !self.api_key.trim().is_empty() {
            Some(self.api_key.trim().to_owned())
        } else {
            None
        };
        // Saving is a convenience: failures are shown but do not block dictation.
        let mut problems = vec![];
        if let Some(path) = &self.save_path
            && let Err(error) = config.save(path)
        {
            problems.push(format!("{error:#}"));
        }
        match (&key, self.remember_key) {
            (Some(key), true) => match windows::credential::save(key) {
                Ok(()) => self.key_saved = true,
                Err(error) => problems.push(format!("{error:#}")),
            },
            (_, false) if self.key_saved => match windows::credential::delete() {
                Ok(()) => self.key_saved = false,
                Err(error) => problems.push(format!("{error:#}")),
            },
            _ => {}
        }
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let (tx, events) = mpsc::channel();
        let ctx = ctx.clone();
        self.error = (!problems.is_empty()).then(|| problems.join(" "));
        self.ready = false;
        self.status = "Starting dictation…".into();
        let worker = thread::spawn(move || {
            let result = (|| {
                let pipeline = match key {
                    Some(key) => Pipeline::with_api_key(config.clone(), Some(key)),
                    None => Pipeline::new(config.clone()),
                }
                .context(
                    "Could not enable dictation. Enter an API key or check your provider settings",
                )?;
                windows::run_controlled(config, pipeline, &signal, |message| {
                    let _ = tx.send(Event::Status(message.to_owned()));
                    ctx.request_repaint();
                })
            })();
            let _ = tx.send(Event::Finished(
                result.map_err(|error| format!("{error:#}")),
            ));
            ctx.request_repaint();
        });
        self.worker = Some(Worker {
            stop,
            events,
            thread: worker,
        });
    }

    fn poll(&mut self) {
        if let Some(worker) = &self.worker {
            // Observe completion before draining so a final error cannot be lost
            // between the last try_recv and removing the worker.
            let finished = worker.thread.is_finished();
            while let Ok(event) = worker.events.try_recv() {
                match event {
                    Event::Status(message) => {
                        self.ready = true;
                        self.status = message;
                    }
                    Event::Finished(result) => {
                        self.ready = false;
                        self.status =
                            "Dictation disabled. No new audio will be recorded or inserted.".into();
                        self.error = result.err();
                    }
                }
            }
            if finished {
                let worker = self.worker.take().unwrap();
                if worker.thread.join().is_err() {
                    self.ready = false;
                    self.error = Some(
                        "The dictation worker stopped unexpectedly. You can try enabling it again."
                            .into(),
                    );
                }
            }
        }
    }

    fn stop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Release);
            self.status = "Stopping… Pending results will not be inserted. Requests already sent cannot be recalled.".into();
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        self.stop();
    }
}

impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|input| input.viewport().close_requested()) {
            self.stop();
        }
        self.poll();
        if self.worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(248, 250, 253)).inner_margin(24.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading(RichText::new("SpeakCraft").size(32.0).strong());
                    ui.label("Your voice, wherever you type.");
                    ui.add_space(8.0);
                    egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(if self.ready { "Dictation enabled" } else { "Dictation disabled" });
                            if self.worker.is_some() {
                                let stopping = self.worker.as_ref().unwrap().stop.load(Ordering::Acquire);
                                if ui.add_enabled(!stopping, egui::Button::new(RichText::new("Disable dictation").color(Color32::WHITE))
                                    .fill(Color32::from_rgb(165, 32, 42))).clicked() {
                                    self.stop();
                                }
                            } else if ui.add(egui::Button::new(RichText::new("Enable dictation").color(Color32::WHITE))
                                .fill(Color32::from_rgb(32, 103, 220))).clicked() {
                                self.start(ctx);
                            }
                        });
                        ui.label(&self.status);
                        if let Some(error) = &self.error {
                            ui.colored_label(Color32::from_rgb(165, 32, 42), error);
                        }
                    });
                    ui.add_space(6.0);
                    ui.add_enabled_ui(self.worker.is_none(), |ui| {
                        let label = ui.label("Provider URL");
                        ui.add(egui::TextEdit::singleline(&mut self.config.base_url).desired_width(f32::INFINITY)).labelled_by(label.id);
                        let label = ui.label("Transcription model");
                        ui.add(egui::TextEdit::singleline(&mut self.config.transcription_model).desired_width(f32::INFINITY)).labelled_by(label.id);
                        ui.checkbox(&mut self.authenticated, "Provider requires an API key");
                        if self.authenticated {
                            let label = ui.label("API key");
                            ui.add(egui::TextEdit::singleline(&mut self.api_key).password(true)
                                .hint_text("Paste your API key here").desired_width(f32::INFINITY)).labelled_by(label.id);
                            ui.checkbox(&mut self.remember_key, "Remember this key on this PC (Windows Credential Manager)");
                            let env = if self.config.api_key_env.is_empty() { "OPENAI_API_KEY" } else { &self.config.api_key_env };
                            ui.small(if self.remember_key {
                                "Saved for your Windows account when you enable dictation; never written to the settings file.".to_owned()
                            } else {
                                format!("Used for this session only. Leave blank to use {env} from the environment.")
                            });
                        }
                        let label = ui.label("Language (optional)");
                        ui.add(egui::TextEdit::singleline(&mut self.language)
                            .hint_text("Auto-detect, or a two-letter code such as en or de").desired_width(f32::INFINITY)).labelled_by(label.id);
                        ui.checkbox(&mut self.config.cleanup, "Clean up transcription with AI (additional request)");
                        if self.config.cleanup {
                            let label = ui.label("Cleanup model");
                            ui.add(egui::TextEdit::singleline(&mut self.config.cleanup_model).desired_width(f32::INFINITY)).labelled_by(label.id);
                        }
                        let label = ui.label("Microphone");
                        ui.horizontal(|ui| {
                            let selected = match &self.config.microphone {
                                None => "Windows default".to_owned(),
                                Some(name) if self.microphones.contains(name) => name.clone(),
                                Some(name) => format!("{name} (not connected)"),
                            };
                            egui::ComboBox::from_id_salt("microphone")
                                .selected_text(selected)
                                .width(ui.available_width() - 90.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.config.microphone, None, "Windows default");
                                    for name in &self.microphones {
                                        ui.selectable_value(&mut self.config.microphone, Some(name.clone()), name);
                                    }
                                })
                                .response
                                .labelled_by(label.id);
                            if ui.button("Refresh").clicked() {
                                self.microphones = windows::microphones();
                            }
                        });
                        if self.microphones.is_empty() {
                            ui.small("No microphone found. Connect one, then click Refresh.");
                        }
                        ui.checkbox(&mut self.config.sounds, "Play sounds when recording starts and stops");
                        ui.checkbox(&mut self.config.overlay, "Show recording status on screen");
                        ui.label("Insert text by");
                        ui.radio_value(&mut self.config.insertion, Insertion::Type, "Typing: clipboard untouched; about 100 characters per second");
                        ui.radio_value(&mut self.config.insertion, Insertion::Paste, "Pasting: instant; previous clipboard text is restored");
                    });
                    ui.add_space(6.0);
                    egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.strong("Dictate in another app");
                        ui.label("Enable dictation, then focus the text field where you want to write.");
                        egui::Grid::new("shortcuts").spacing([24.0, 8.0]).show(ui, |ui| {
                            for (key, action) in [
                                ("Ctrl + Alt + Space", "Hold to talk, or tap to start / finish"),
                                ("Ctrl + Alt + Esc", "Cancel / clear retained text"),
                                ("Ctrl + Alt + V", "Retry retained text insertion"),
                            ] {
                                ui.label(RichText::new(key).strong());
                                ui.label(action);
                                ui.end_row();
                            }
                        });
                    });
                    ui.label("Audio is sent to your configured provider. AI cleanup also sends the transcript. Provider charges and privacy policies apply.");
                    ui.small("Minimize this window to keep dictating. Closing it stops dictation. No tray icon or background autostart.");
                    if self.key_saved {
                        ui.small("An API key is saved in Windows Credential Manager. Uncheck \u{201c}Remember this key\u{201d} and enable dictation to remove it.");
                    }
                    ui.small(&self.source);
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::Role;
    use egui_kittest::{Harness, kittest::Queryable};
    use std::time::Instant;

    fn wait_for(harness: &mut Harness<'_, Desktop>, label: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            harness.step();
            if harness.query_by_label(label).is_some() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Missing UI status: {label}; error: {:?}",
                harness.state().error
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn desktop_reports_setup_errors_and_can_enable_disable_and_restart() {
        let config = Config {
            base_url: "http://127.0.0.1:9/v1".into(),
            api_key_env: "SPEAKCRAFT_TEST_MISSING_CREDENTIAL".into(),
            ..Config::default()
        };
        let mut harness = Harness::builder()
            .with_size([620.0, 820.0])
            .build_eframe(|cc| Desktop::new(Ok(config), None, None, Ok(None), &cc.egui_ctx));
        harness.run();
        harness.get_by_label("Dictation disabled");
        harness.get_by_label("Enable dictation").click();
        wait_for(
            &mut harness,
            "Dictation disabled. No new audio will be recorded or inserted.",
        );
        harness
            .get_by_label_contains("Set the SPEAKCRAFT_TEST_MISSING_CREDENTIAL environment variable");
        wait_for(&mut harness, "Enable dictation");

        harness
            .get_by_role_and_label(Role::TextInput, "Language (optional)")
            .click();
        harness.run();
        harness
            .get_by_role_and_label(Role::TextInput, "Language (optional)")
            .type_text("invalid");
        harness.run();
        harness.get_by_label("Enable dictation").click();
        harness.run();
        harness.get_by_label("language must be a lowercase two-letter ISO-639-1 code");
        harness
            .get_by_role_and_label(Role::TextInput, "Language (optional)")
            .click();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.key_press(egui::Key::Backspace);
        harness.run();

        harness
            .get_by_role_and_label(Role::PasswordInput, "API key")
            .click();
        harness.run();
        harness
            .get_by_role_and_label(Role::PasswordInput, "API key")
            .type_text("local-test-credential");
        harness.run();
        for _ in 0..2 {
            harness.get_by_label("Enable dictation").click();
            wait_for(
                &mut harness,
                "Ready. Focus an editable text field before starting. Do not change fields until insertion finishes.",
            );
            harness.get_by_label("Dictation enabled");
            harness.get_by_label("Disable dictation").click();
            wait_for(
                &mut harness,
                "Dictation disabled. No new audio will be recorded or inserted.",
            );
            wait_for(&mut harness, "Enable dictation");
        }
    }
}
