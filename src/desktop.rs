use crate::{config::Config, pipeline::Pipeline, windows};
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
    let path = config_path.or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("config.toml")))
            .filter(|path| path.exists())
    });
    let config = Config::load(path.as_deref());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Saykey")
            .with_inner_size([620.0, 820.0])
            .with_min_inner_size([440.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Saykey",
        options,
        Box::new(move |cc| Ok(Box::new(Desktop::new(config, path, &cc.egui_ctx)))),
    )
    .map_err(|error| anyhow::anyhow!("Cannot open Saykey desktop window: {error}"))
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
    language: String,
    api_key: String,
    authenticated: bool,
    worker: Option<Worker>,
    ready: bool,
    status: String,
    error: Option<String>,
}

impl Desktop {
    fn new(config: Result<Config>, path: Option<PathBuf>, ctx: &egui::Context) -> Self {
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
        let (config, error) = match config {
            Ok(config) => (config, None),
            Err(error) => (
                Config::default(),
                Some(format!(
                    "Could not load configuration: {error:#}. Defaults are shown; review the settings before enabling dictation."
                )),
            ),
        };
        Self {
            source: path.map_or_else(
                || "Using built-in defaults. Settings below apply to this session only.".into(),
                |path| {
                    format!(
                        "Configuration: {}. Edits apply to this session only.",
                        path.display()
                    )
                },
            ),
            language: config.language.clone().unwrap_or_default(),
            authenticated: !config.api_key_env.is_empty(),
            config,
            api_key: String::new(),
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
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let (tx, events) = mpsc::channel();
        let ctx = ctx.clone();
        self.error = None;
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
                    ui.heading(RichText::new("Saykey").size(32.0).strong());
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
                            let label = ui.label("API key — this session only");
                            ui.add(egui::TextEdit::singleline(&mut self.api_key).password(true)
                                .hint_text("Paste your API key here").desired_width(f32::INFINITY)).labelled_by(label.id);
                            let env = if self.config.api_key_env.is_empty() { "OPENAI_API_KEY" } else { &self.config.api_key_env };
                            ui.small(format!("Not saved to disk. Leave blank to use {env} from the environment."));
                        }
                        let label = ui.label("Language (optional)");
                        ui.add(egui::TextEdit::singleline(&mut self.language)
                            .hint_text("Auto-detect, or a two-letter code such as en or de").desired_width(f32::INFINITY)).labelled_by(label.id);
                        ui.checkbox(&mut self.config.cleanup, "Clean up transcription with AI (additional request)");
                        if self.config.cleanup {
                            let label = ui.label("Cleanup model");
                            ui.add(egui::TextEdit::singleline(&mut self.config.cleanup_model).desired_width(f32::INFINITY)).labelled_by(label.id);
                        }
                    });
                    ui.add_space(6.0);
                    egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.strong("Dictate in another app");
                        ui.label("Enable dictation, then focus the text field where you want to write.");
                        egui::Grid::new("shortcuts").spacing([24.0, 8.0]).show(ui, |ui| {
                            for (key, action) in [
                                ("Ctrl + Alt + Space", "Start / finish recording"),
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
            api_key_env: "SAYKEY_TEST_MISSING_CREDENTIAL".into(),
            ..Config::default()
        };
        let mut harness = Harness::builder()
            .with_size([620.0, 820.0])
            .build_eframe(|cc| Desktop::new(Ok(config), None, &cc.egui_ctx));
        harness.run();
        harness.get_by_label("Dictation disabled");
        harness.get_by_label("Enable dictation").click();
        wait_for(
            &mut harness,
            "Dictation disabled. No new audio will be recorded or inserted.",
        );
        harness
            .get_by_label_contains("Set the SAYKEY_TEST_MISSING_CREDENTIAL environment variable");
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
            .get_by_role_and_label(Role::PasswordInput, "API key — this session only")
            .click();
        harness.run();
        harness
            .get_by_role_and_label(Role::PasswordInput, "API key — this session only")
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
