//! Settings window (egui). Edits ~/.config/index01/watch.env and controls the service.

use crate::config::Config;
use crate::service;
use eframe::egui;
use std::sync::mpsc;

const PASTE_KEYS: [(&str, &str); 3] = [
    ("ctrl-v", "Ctrl+V (most apps)"),
    ("ctrl-shift-v", "Ctrl+Shift+V (terminals)"),
    ("shift-insert", "Shift+Insert"),
];

type ScanResult = Result<Vec<(String, String)>, String>;

pub struct Settings {
    cfg: Config,
    saved: Config,
    models: Vec<String>,
    state: String,
    transcripts: Vec<String>,
    log_error: Option<String>,
    message: Option<(bool, String)>,
    scan_rx: Option<mpsc::Receiver<ScanResult>>,
    rings: Vec<(String, String)>,
    last_poll: f64,
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Index 01")
            .with_app_id("index01-tray")
            .with_inner_size([520.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Index 01",
        options,
        Box::new(|_cc| Ok(Box::new(Settings::new()))),
    )
}

impl Settings {
    fn new() -> Self {
        let cfg = Config::load();
        let (transcripts, log_error) = service::recent_log();
        Settings {
            saved: cfg.clone(),
            cfg,
            models: service::local_models(),
            state: service::state(),
            transcripts,
            log_error,
            message: None,
            scan_rx: None,
            rings: vec![],
            last_poll: 0.0,
        }
    }

    fn save(&mut self) {
        let res = self.cfg.save().and_then(|_| service::reload_if_active());
        self.message = Some(match res {
            Ok(()) => {
                self.saved = self.cfg.clone();
                (
                    true,
                    if self.state == "active" {
                        "Saved; service restarted."
                    } else {
                        "Saved."
                    }
                    .into(),
                )
            }
            Err(e) => (false, format!("Save failed: {e}")),
        });
    }

    fn text_row(ui: &mut egui::Ui, cfg: &mut Config, key: &str, label: &str, hint: &str) {
        ui.label(label);
        let mut v = cfg.get(key).to_owned();
        if ui
            .add(
                egui::TextEdit::singleline(&mut v)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            cfg.set(key, v);
        }
        ui.end_row();
    }
}

impl eframe::App for Settings {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Poll service status every 2 s.
        let now = ui.input(|i| i.time);
        if now - self.last_poll > 2.0 {
            self.last_poll = now;
            self.state = service::state();
            (self.transcripts, self.log_error) = service::recent_log();
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(2));
        if let Some(rx) = &self.scan_rx
            && let Ok(r) = rx.try_recv()
        {
            self.scan_rx = None;
            match r {
                Ok(v) if v.is_empty() => {
                    self.message =
                        Some((false, "No ring found. Tap it to wake it and retry.".into()))
                }
                Ok(v) => {
                    if self.cfg.get("INDEX01_ADDRESS").is_empty() {
                        self.cfg.set("INDEX01_ADDRESS", &v[0].0);
                    }
                    self.rings = v;
                    self.message = None;
                }
                Err(e) => self.message = Some((false, format!("Scan failed: {e}"))),
            }
        }

        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.body(ui));
        });
    }
}

impl Settings {
    fn body(&mut self, ui: &mut egui::Ui) {
        // Service
        ui.heading("Service");
        ui.horizontal(|ui| {
            let (color, text) = match self.state.as_str() {
                "active" => (egui::Color32::from_rgb(80, 180, 90), "Listening"),
                "activating" => (egui::Color32::from_rgb(200, 170, 60), "Starting"),
                "failed" => (egui::Color32::from_rgb(210, 80, 70), "Failed"),
                _ => (ui.visuals().weak_text_color(), "Off"),
            };
            ui.colored_label(color, "●");
            ui.label(text);
            ui.add_space(12.0);
            let running = self.state == "active" || self.state == "activating";
            let has_addr = !self.saved.get("INDEX01_ADDRESS").is_empty();
            let btn = if running { "Stop" } else { "Start" };
            let resp = ui.add_enabled(has_addr || running, egui::Button::new(btn));
            if resp.clicked() {
                if let Err(e) = service::set_enabled(!running) {
                    self.message = Some((false, format!("service: {e}")));
                }
                self.state = service::state();
            }
            if !has_addr {
                ui.weak("Set and save a ring address first.");
            }
        });
        ui.weak(
            "Runs `index01 watch` as the systemd user service index01-watch, started at login.",
        );
        if let Some(e) = &self.log_error {
            ui.colored_label(egui::Color32::from_rgb(210, 80, 70), e);
        }
        ui.add_space(8.0);

        // Ring
        ui.heading("Ring");
        egui::Grid::new("ring")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("Address");
                ui.horizontal(|ui| {
                    let mut v = self.cfg.get("INDEX01_ADDRESS").to_owned();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut v)
                                .hint_text("AA:BB:CC:DD:EE:FF")
                                .desired_width(170.0),
                        )
                        .changed()
                    {
                        self.cfg.set("INDEX01_ADDRESS", v.trim().to_uppercase());
                    }
                    if self.scan_rx.is_some() {
                        ui.spinner();
                        ui.label("Scanning…");
                    } else if ui
                        .button("Scan")
                        .on_hover_text("Tap the ring first to wake it")
                        .clicked()
                    {
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(service::scan_rings(8));
                        });
                        self.scan_rx = Some(rx);
                    }
                });
                ui.end_row();
                for (addr, line) in self.rings.clone() {
                    ui.label("");
                    if ui
                        .selectable_label(self.cfg.get("INDEX01_ADDRESS") == addr, &line)
                        .clicked()
                    {
                        self.cfg.set("INDEX01_ADDRESS", &addr);
                    }
                    ui.end_row();
                }
                Self::text_row(
                    ui,
                    &mut self.cfg,
                    "INDEX01_OUT",
                    "Recordings folder",
                    "~/Index01",
                );
                ui.label("");
                let mut keep = self.cfg.flag("INDEX01_KEEP");
                if ui
                    .checkbox(
                        &mut keep,
                        "Keep recordings on the ring (don't erase after download)",
                    )
                    .changed()
                {
                    self.cfg.set_flag("INDEX01_KEEP", keep);
                }
                ui.end_row();
            });
        ui.add_space(8.0);

        // Transcription
        ui.heading("Transcription");
        egui::Grid::new("stt").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Backend");
            let mut backend = self.cfg.get("INDEX01_BACKEND").to_owned();
            if backend.is_empty() {
                backend = "local".into();
            }
            egui::ComboBox::from_id_salt("backend")
                .selected_text(match backend.as_str() {
                    "local" => "Local model (in-process)",
                    "openai" => "OpenAI-compatible server",
                    "command" => "Command",
                    other => other,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut backend, "local".into(), "Local model (in-process)");
                    ui.selectable_value(&mut backend, "openai".into(), "OpenAI-compatible server");
                    ui.selectable_value(&mut backend, "command".into(), "Command");
                });
            self.cfg.set("INDEX01_BACKEND", &backend);
            ui.end_row();

            match backend.as_str() {
                "local" => {
                    ui.label("Model");
                    let mut m = self.cfg.get("INDEX01_MODEL_DIR").to_owned();
                    if m.is_empty() {
                        m = "auto".into();
                    }
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("model").selected_text(&m).width(220.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut m, "auto".into(), "auto (Parakeet v2 first)");
                            for spec in &self.models {
                                ui.selectable_value(&mut m, spec.clone(), spec);
                            }
                        });
                        if ui.small_button("Folder…").on_hover_text("Use any ONNX model directory").clicked()
                            && let Some(d) = pick_folder() {
                                m = d;
                            }
                    });
                    self.cfg.set("INDEX01_MODEL_DIR", &m);
                    ui.end_row();
                    ui.label("");
                    ui.weak("Reuses models downloaded by Handy and Spokenly.\nParakeet, Canary, Cohere, SenseVoice and GigaAM are supported.");
                    ui.end_row();
                }
                "openai" => {
                    Self::text_row(ui, &mut self.cfg, "INDEX01_URL", "Server URL", "http://localhost:8000/v1");
                    Self::text_row(ui, &mut self.cfg, "INDEX01_MODEL", "Model", "openai/whisper-large-v3-turbo");
                    Self::text_row(ui, &mut self.cfg, "INDEX01_API_KEY", "API key", "(optional)");
                }
                _ => {
                    Self::text_row(ui, &mut self.cfg, "INDEX01_COMMAND", "Command", "python3 my_stt.py  (gets the WAV path)");
                }
            }
            Self::text_row(ui, &mut self.cfg, "INDEX01_LANGUAGE", "Language", "auto, or e.g. en");
            Self::text_row(ui, &mut self.cfg, "INDEX01_PROMPT", "Vocabulary hint", "(optional, server backends)");
        });
        ui.add_space(8.0);

        // Output
        ui.heading("Output");
        egui::Grid::new("out")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("");
                let mut paste = self.cfg.flag("INDEX01_PASTE");
                if ui
                    .checkbox(&mut paste, "Paste into the focused window")
                    .changed()
                {
                    self.cfg.set_flag("INDEX01_PASTE", paste);
                }
                ui.end_row();
                ui.label("Paste shortcut");
                let mut k = self.cfg.get("INDEX01_PASTE_KEYS").to_owned();
                if k.is_empty() {
                    k = "ctrl-v".into();
                }
                let label = PASTE_KEYS
                    .iter()
                    .find(|p| p.0 == k)
                    .map(|p| p.1)
                    .unwrap_or(&k)
                    .to_owned();
                egui::ComboBox::from_id_salt("keys")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (v, l) in PASTE_KEYS {
                            ui.selectable_value(&mut k, v.into(), l);
                        }
                    });
                self.cfg.set("INDEX01_PASTE_KEYS", &k);
                ui.end_row();
                Self::text_row(
                    ui,
                    &mut self.cfg,
                    "INDEX01_EXEC",
                    "Also run",
                    "shell command; text on stdin, e.g. cat >> ~/notes.md",
                );
            });
        ui.add_space(10.0);

        // Save
        ui.horizontal(|ui| {
            let dirty = self.cfg != self.saved;
            if ui.add_enabled(dirty, egui::Button::new("Save")).clicked() {
                self.save();
            }
            if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
                self.cfg = self.saved.clone();
            }
            if let Some((ok, m)) = &self.message {
                let c = if *ok {
                    egui::Color32::from_rgb(80, 180, 90)
                } else {
                    egui::Color32::from_rgb(210, 80, 70)
                };
                ui.colored_label(c, m);
            }
        });
        ui.add_space(10.0);

        // Recent
        ui.heading("Recent transcripts");
        if self.transcripts.is_empty() {
            ui.weak("None yet. Hold the ring button, speak, and release.");
        }
        for t in self.transcripts.iter().rev() {
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Copy").clicked() {
                    service::copy(t);
                }
                ui.label(t);
            });
        }
    }
}

/// Folder picker via zenity (GNOME); None if unavailable or cancelled.
fn pick_folder() -> Option<String> {
    let o = std::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=Choose a model folder",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();
    (o.status.success() && !s.is_empty()).then_some(s)
}
