//! Settings window (egui). Edits ~/.config/index01/watch.env and controls the service.

use crate::config::Config;
use crate::service;
use crate::style;
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
    /// Whether this computer is bonded with the saved ring.
    paired: bool,
    /// A running pair/unpair job.
    job_rx: Option<mpsc::Receiver<Result<String, String>>>,
    confirm_unpair: bool,
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Index 01")
            .with_app_id("index01-tray")
            .with_inner_size([640.0, 760.0])
            .with_min_inner_size([420.0, 400.0])
            // GNOME has no server-side decorations; winit's fallback frame looks foreign.
            // We draw our own: rounded corners, border, header bar with close button.
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native(
        "Index 01",
        options,
        Box::new(|cc| {
            style::apply(&cc.egui_ctx);
            Ok(Box::new(Settings::new()))
        }),
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
            paired: service::is_paired(cfg_addr(&Config::load())),
            job_rx: None,
            confirm_unpair: false,
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
}

impl eframe::App for Settings {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Poll service status every 2 s.
        let now = ui.input(|i| i.time);
        if now - self.last_poll > 2.0 {
            self.last_poll = now;
            self.state = service::state();
            (self.transcripts, self.log_error) = service::recent_log();
            if self.job_rx.is_none() {
                self.paired = service::is_paired(cfg_addr(&self.saved));
            }
        }
        if let Some(rx) = &self.job_rx
            && let Ok(r) = rx.try_recv()
        {
            self.job_rx = None;
            self.paired = service::is_paired(cfg_addr(&self.saved));
            self.state = service::state();
            self.message = Some(match r {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            });
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

        debug_screenshot(ui.ctx());
        let p = style::Palette::get(ui.ctx());
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        let radius = if maximized { 0 } else { 12 };
        let window = ui.max_rect();
        egui::Frame::new()
            .fill(p.window)
            .stroke(egui::Stroke::new(1.0, p.card_border))
            .corner_radius(radius)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                self.header(ui, radius);
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        // libadwaita "clamp": centred column, max 600 px.
                        let pad = ((ui.available_width() - 600.0) / 2.0).max(18.0);
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: pad as i8,
                                right: pad as i8,
                                top: 4,
                                bottom: 24,
                            })
                            .show(ui, |ui| self.body(ui));
                    });
            });
        if !maximized {
            resize_handles(ui, window);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // transparent, so the rounded corners show the desktop
    }
}

/// Edge/corner resize for an undecorated window.
fn resize_handles(ui: &mut egui::Ui, r: egui::Rect) {
    use egui::{CursorIcon as C, ResizeDirection as D};
    let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let m = 6.0;
    let (l, rt, t, b) = (
        pos.x < r.left() + m,
        pos.x > r.right() - m,
        pos.y < r.top() + m,
        pos.y > r.bottom() - m,
    );
    let dir = match (l, rt, t, b) {
        (true, _, true, _) => (D::NorthWest, C::ResizeNorthWest),
        (_, true, true, _) => (D::NorthEast, C::ResizeNorthEast),
        (true, _, _, true) => (D::SouthWest, C::ResizeSouthWest),
        (_, true, _, true) => (D::SouthEast, C::ResizeSouthEast),
        (true, ..) => (D::West, C::ResizeWest),
        (_, true, ..) => (D::East, C::ResizeEast),
        (_, _, true, _) => (D::North, C::ResizeNorth),
        (.., true) => (D::South, C::ResizeSouth),
        _ => return,
    };
    ui.ctx().set_cursor_icon(dir.1);
    if ui.input(|i| i.pointer.primary_pressed()) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir.0));
    }
}

impl Settings {
    /// Header bar, which is also the title bar: drag to move, double-click to maximize.
    fn header(&mut self, ui: &mut egui::Ui, radius: u8) {
        let p = style::Palette::get(ui.ctx());
        let resp = egui::Frame::new()
            .fill(p.card)
            .corner_radius(egui::CornerRadius {
                nw: radius,
                ne: radius,
                sw: 0,
                se: 0,
            })
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // Background drag area, registered first so the buttons on top win.
                let bar = ui.max_rect().expand2(egui::vec2(10.0, 8.0));
                let drag = ui.interact(
                    bar,
                    ui.scope_id().with("titlebar"),
                    egui::Sense::click_and_drag(),
                );
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                ui.horizontal(|ui| {
                    ui.set_min_height(34.0);
                    let dirty = self.cfg != self.saved;
                    if dirty && ui.button("Revert").clicked() {
                        self.cfg = self.saved.clone();
                        self.message = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if close_button(ui).clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ui.add_space(6.0);
                        if style::action_button(ui, "Save", p.accent, dirty).clicked() {
                            self.save();
                        }
                        if let Some((ok, m)) = &self.message {
                            ui.colored_label(if *ok { p.dim } else { style::ERROR }, m);
                        }
                    });
                });
            })
            .response;
        let r = resp.rect;
        ui.painter().hline(
            r.x_range(),
            r.bottom(),
            egui::Stroke::new(1.0, p.card_border),
        );
        // Window title, centred like a GTK header bar.
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "Index 01",
            egui::FontId::new(15.0, egui::FontFamily::Name("bold".into())),
            p.fg,
        );
    }

    fn text_field(ui: &mut egui::Ui, cfg: &mut Config, key: &str, hint: &str, width: f32) {
        let mut v = cfg.get(key).to_owned();
        if style::entry(ui, &mut v, hint, width).changed() {
            cfg.set(key, v);
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let p = style::Palette::get(ui.ctx());
        let wide = (ui.available_width() * 0.55).clamp(200.0, 320.0);

        // Status
        let running = self.state == "active" || self.state == "activating";
        let has_addr = !self.saved.get("INDEX01_ADDRESS").is_empty();
        style::group(ui, "Service", |ui| {
            let (dot, title, sub) = match self.state.as_str() {
                "active" => (
                    style::SUCCESS,
                    "Listening",
                    "Hold the ring button, speak, release.",
                ),
                "activating" => (style::WARNING, "Starting", "Loading the model…"),
                "failed" => (
                    style::ERROR,
                    "Stopped after an error",
                    "See the message below.",
                ),
                _ if !has_addr => (p.dim, "Not set up", "Choose your ring below, then Save."),
                _ => (p.dim, "Off", "Not listening to the ring."),
            };
            style::row(ui, true, "", None, |ui| {
                let mut on = running;
                if ui
                    .add_enabled_ui(has_addr || running, |ui| style::switch(ui, &mut on))
                    .inner
                    .changed()
                {
                    if let Err(e) = service::set_enabled(on) {
                        self.message = Some((false, format!("service: {e}")));
                    }
                    self.state = service::state();
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, dot);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(egui::RichText::new(title).strong());
                        ui.label(egui::RichText::new(sub).small().color(p.dim));
                    });
                });
            });
            if let Some(e) = &self.log_error {
                style::row(ui, false, "Last error", None, |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(e).small().color(style::ERROR))
                            .truncate(),
                    )
                    .on_hover_text(e);
                });
            }
        });

        // Ring
        style::group(ui, "Ring", |ui| {
            style::row(
                ui,
                true,
                "Address",
                Some("Tap the ring to wake it before scanning"),
                |ui| {
                    if self.scan_rx.is_some() {
                        ui.spinner();
                    } else if ui.button("Scan").clicked() {
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(service::scan_rings(8));
                        });
                        self.scan_rx = Some(rx);
                        self.message = None;
                    }
                    let mut v = self.cfg.get("INDEX01_ADDRESS").to_owned();
                    if style::entry(ui, &mut v, "AA:BB:CC:DD:EE:FF", 170.0).changed() {
                        self.cfg.set("INDEX01_ADDRESS", v.trim().to_uppercase());
                    }
                },
            );
            if has_addr {
                self.pairing_row(ui);
            }
            for (addr, line) in self.rings.clone() {
                let sel = self.cfg.get("INDEX01_ADDRESS") == addr;
                style::row(ui, false, &line, None, |ui| {
                    if sel {
                        ui.label(egui::RichText::new("✔").color(p.accent));
                    } else if ui.button("Use").clicked() {
                        self.cfg.set("INDEX01_ADDRESS", &addr);
                    }
                });
            }
            style::row(ui, false, "Recordings folder", None, |ui| {
                if ui.button("Open").clicked() {
                    let out = std::path::PathBuf::from(self.cfg.get("INDEX01_OUT"));
                    let _ = std::fs::create_dir_all(&out);
                    service::open(&out);
                }
                Self::text_field(ui, &mut self.cfg, "INDEX01_OUT", "~/Index01", wide - 70.0);
            });
            style::row(
                ui,
                false,
                "Keep recordings on the ring",
                Some("Otherwise they are erased after download"),
                |ui| {
                    let mut keep = self.cfg.flag("INDEX01_KEEP");
                    if style::switch(ui, &mut keep).changed() {
                        self.cfg.set_flag("INDEX01_KEEP", keep);
                    }
                },
            );
        });

        // Transcription
        style::group(ui, "Transcription", |ui| {
            let mut backend = self.cfg.get("INDEX01_BACKEND").to_owned();
            if backend.is_empty() {
                backend = "local".into();
            }
            style::row(ui, true, "Engine", None, |ui| {
                egui::ComboBox::from_id_salt("backend")
                    .width(wide)
                    .selected_text(match backend.as_str() {
                        "local" => "Local model",
                        "openai" => "OpenAI-compatible server",
                        "command" => "Custom command",
                        other => other,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut backend, "local".into(), "Local model");
                        ui.selectable_value(
                            &mut backend,
                            "openai".into(),
                            "OpenAI-compatible server",
                        );
                        ui.selectable_value(&mut backend, "command".into(), "Custom command");
                    });
            });
            self.cfg.set("INDEX01_BACKEND", &backend);

            match backend.as_str() {
                "local" => {
                    let mut m = self.cfg.get("INDEX01_MODEL_DIR").to_owned();
                    if m.is_empty() {
                        m = "auto".into();
                    }
                    style::row(
                        ui,
                        false,
                        "Model",
                        Some("Reuses Handy and Spokenly downloads"),
                        |ui| {
                            if ui
                                .button("Browse…")
                                .on_hover_text("Any ONNX model folder")
                                .clicked()
                                && let Some(d) = pick_folder()
                            {
                                m = d;
                            }
                            let shown = if m == "auto" {
                                "Automatic".to_owned()
                            } else {
                                m.clone()
                            };
                            egui::ComboBox::from_id_salt("model")
                                .width(wide - 90.0)
                                .selected_text(shown)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut m,
                                        "auto".into(),
                                        "Automatic (Parakeet v2 first)",
                                    );
                                    for spec in &self.models {
                                        ui.selectable_value(&mut m, spec.clone(), spec);
                                    }
                                });
                        },
                    );
                    self.cfg.set("INDEX01_MODEL_DIR", &m);
                }
                "openai" => {
                    style::row(ui, false, "Server URL", None, |ui| {
                        Self::text_field(
                            ui,
                            &mut self.cfg,
                            "INDEX01_URL",
                            "http://localhost:8000/v1",
                            wide,
                        )
                    });
                    style::row(ui, false, "Model", None, |ui| {
                        Self::text_field(
                            ui,
                            &mut self.cfg,
                            "INDEX01_MODEL",
                            "openai/whisper-large-v3-turbo",
                            wide,
                        )
                    });
                    style::row(ui, false, "API key", Some("Optional"), |ui| {
                        Self::text_field(ui, &mut self.cfg, "INDEX01_API_KEY", "", wide)
                    });
                    style::row(
                        ui,
                        false,
                        "Vocabulary hint",
                        Some("Optional prompt"),
                        |ui| Self::text_field(ui, &mut self.cfg, "INDEX01_PROMPT", "", wide),
                    );
                }
                _ => {
                    style::row(
                        ui,
                        false,
                        "Command",
                        Some("Gets the WAV path as $1, prints text"),
                        |ui| {
                            Self::text_field(
                                ui,
                                &mut self.cfg,
                                "INDEX01_COMMAND",
                                "python3 stt.py",
                                wide,
                            )
                        },
                    );
                }
            }
            style::row(ui, false, "Language", None, |ui| {
                Self::text_field(ui, &mut self.cfg, "INDEX01_LANGUAGE", "Automatic", 120.0)
            });
        });

        // Output
        style::group(ui, "Output", |ui| {
            style::row(ui, true, "Paste into the focused window", None, |ui| {
                let mut paste = self.cfg.flag("INDEX01_PASTE");
                if style::switch(ui, &mut paste).changed() {
                    self.cfg.set_flag("INDEX01_PASTE", paste);
                }
            });
            if self.cfg.flag("INDEX01_PASTE") {
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
                style::row(ui, false, "Paste shortcut", None, |ui| {
                    egui::ComboBox::from_id_salt("keys")
                        .width(wide)
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            for (v, l) in PASTE_KEYS {
                                ui.selectable_value(&mut k, v.into(), l);
                            }
                        });
                });
                self.cfg.set("INDEX01_PASTE_KEYS", &k);
            }
            style::row(
                ui,
                false,
                "Also run",
                Some("Shell command, transcript on stdin"),
                |ui| Self::text_field(ui, &mut self.cfg, "INDEX01_EXEC", "cat >> ~/notes.md", wide),
            );
        });

        // Recent
        style::group(ui, "Recent transcripts", |ui| {
            if self.transcripts.is_empty() {
                style::row(ui, true, "Nothing yet", None, |_| {});
            }
            for (i, t) in self.transcripts.iter().rev().enumerate() {
                if i > 0 {
                    let y = ui.cursor().top();
                    let x = ui.max_rect().x_range();
                    ui.painter().hline(
                        egui::Rangef::new(x.min - 14.0, x.max + 14.0),
                        y,
                        egui::Stroke::new(1.0, p.card_border),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copy").clicked() {
                            service::copy(t);
                        }
                        ui.with_layout(
                            egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                            |ui| {
                                ui.label(t);
                            },
                        );
                    });
                });
                ui.add_space(10.0);
            }
        });
    }
}

/// Folder picker via zenity (GNOME); None if unavailable or cancelled.
fn cfg_addr(cfg: &Config) -> &str {
    cfg.get("INDEX01_ADDRESS")
}

impl Settings {
    /// Pair with / unpair from the saved ring, so it can move between computers.
    fn pairing_row(&mut self, ui: &mut egui::Ui) {
        let addr = cfg_addr(&self.saved).to_owned();
        let (title, sub) = if self.paired {
            (
                "Paired with this computer",
                "Unpair to use the ring with another computer or phone (no SOS reset needed)",
            )
        } else {
            (
                "Not paired with this computer",
                "Press the ring button, then Pair. If it is paired elsewhere, unpair it there first",
            )
        };
        style::row(ui, false, title, Some(sub), |ui| {
            if self.job_rx.is_some() {
                ui.spinner();
                return;
            }
            let run = |f: fn(&str) -> Result<String, String>| {
                let (tx, rx) = mpsc::channel();
                let addr = addr.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(f(&addr));
                });
                rx
            };
            if !self.paired {
                if style::action_button(ui, "Pair", style::accent(), true).clicked() {
                    self.job_rx = Some(run(|a| {
                        let m = service::pair(a)?;
                        service::set_enabled(true).map_err(|e| e.to_string())?;
                        Ok(m + " Listening.")
                    }));
                    self.message = Some((true, "Pairing… press the ring button.".into()));
                }
            } else if self.confirm_unpair {
                if style::action_button(ui, "Unpair", style::ERROR, true).clicked() {
                    self.confirm_unpair = false;
                    self.job_rx = Some(run(service::unpair));
                    self.message = Some((true, "Unpairing… press the ring button.".into()));
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_unpair = false;
                }
            } else if ui.button("Unpair…").clicked() {
                self.confirm_unpair = true;
            }
        });
    }
}

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

/// INDEX01_TRAY_SCREENSHOT=/path.ppm: save one frame and exit (for docs and visual checks).
fn debug_screenshot(ctx: &egui::Context) {
    let Some(path) = std::env::var_os("INDEX01_TRAY_SCREENSHOT") else {
        return;
    };
    ctx.request_repaint();
    if ctx.cumulative_frame_nr() == 20 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
            egui::ScreenshotCallback::new(move |img| {
                let mut out = format!("P6 {} {} 255\n", img.size[0], img.size[1]).into_bytes();
                for px in &img.pixels {
                    out.extend_from_slice(&[px.r(), px.g(), px.b()]);
                }
                let _ = std::fs::write(&path, out);
                std::process::exit(0);
            }),
        ));
    }
}

/// GNOME-style round close button (the ✕ is drawn; Adwaita Sans lacks the glyph).
fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let p = style::Palette::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        p.control_hover.gamma_multiply(1.3)
    } else if resp.hovered() {
        p.control_hover
    } else {
        p.control
    };
    let c = rect.center();
    ui.painter().circle_filled(c, 12.0, fill);
    let s = egui::Stroke::new(1.6, p.fg);
    let d = 4.0;
    ui.painter()
        .line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], s);
    ui.painter()
        .line_segment([c + egui::vec2(-d, d), c + egui::vec2(d, -d)], s);
    resp.on_hover_text("Close")
}
