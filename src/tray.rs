//! Status icon (StatusNotifierItem). On GNOME this needs the AppIndicator extension.

use crate::config::Config;
use crate::service;
use ksni::menu::*;
use ksni::{MenuItem, ToolTip, Tray};

/// Everything the tray shows, read from systemd, the journal and the settings file.
#[derive(Clone, PartialEq, Default)]
pub struct Snapshot {
    pub state: String,
    pub transcripts: Vec<String>,
    pub error: Option<String>,
    pub cfg: Config,
}

impl Snapshot {
    pub fn read() -> Self {
        let (transcripts, error) = service::recent_log();
        Snapshot {
            state: service::state(),
            transcripts,
            error,
            cfg: Config::load(),
        }
    }
}

pub struct RingTray {
    pub state: String,
    pub transcripts: Vec<String>,
    pub error: Option<String>,
    pub cfg: Config,
    pub models: Vec<String>,
}

impl RingTray {
    pub fn new() -> Self {
        let mut t = RingTray {
            state: String::new(),
            transcripts: vec![],
            error: None,
            cfg: Config::default(),
            models: service::local_models(),
        };
        t.apply(Snapshot::read());
        t
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.state.clone(),
            transcripts: self.transcripts.clone(),
            error: self.error.clone(),
            cfg: self.cfg.clone(),
        }
    }

    pub fn apply(&mut self, s: Snapshot) {
        self.state = s.state;
        self.transcripts = s.transcripts;
        self.error = s.error;
        self.cfg = s.cfg;
    }

    fn running(&self) -> bool {
        self.state == "active" || self.state == "activating"
    }

    fn status_line(&self) -> String {
        let addr = self.cfg.get("INDEX01_ADDRESS");
        match self.state.as_str() {
            _ if addr.is_empty() => "No ring set: open Settings".into(),
            "active" => format!("Listening to {addr}"),
            "activating" => "Starting…".into(),
            "failed" => "Stopped (failed): see Settings".into(),
            _ => "Off".into(),
        }
    }

    /// Change one setting, save, and restart the service if running.
    fn update_cfg(&mut self, f: impl FnOnce(&mut Config)) {
        f(&mut self.cfg);
        if let Err(e) = self.cfg.save().and_then(|_| service::reload_if_active()) {
            self.error = Some(format!("saving settings: {e}"));
        }
    }
}

fn shorten(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

pub fn open_settings() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("settings").spawn();
    }
}

impl Tray for RingTray {
    fn id(&self) -> String {
        "index01-tray".into()
    }

    fn title(&self) -> String {
        "Index 01".into()
    }

    fn icon_name(&self) -> String {
        match self.state.as_str() {
            "active" => "audio-input-microphone-symbolic",
            "failed" => "dialog-warning-symbolic",
            _ => "microphone-disabled-symbolic",
        }
        .into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: format!("Index 01: {}", self.status_line()),
            description: self.transcripts.last().cloned().unwrap_or_default(),
            ..Default::default()
        }
    }

    // Left click opens settings; the menu is on right click.
    fn activate(&mut self, _x: i32, _y: i32) {
        open_settings();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: self.status_line(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];
        if let Some(e) = &self.error {
            items.push(
                StandardItem {
                    label: shorten(e, 70),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(MenuItem::Separator);

        if self.transcripts.is_empty() {
            items.push(
                StandardItem {
                    label: "No transcripts yet".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        } else {
            for t in self.transcripts.iter().rev() {
                let text = t.clone();
                items.push(
                    StandardItem {
                        label: shorten(t, 60),
                        icon_name: "edit-copy-symbolic".into(),
                        activate: Box::new(move |_: &mut Self| service::copy(&text)),
                        ..Default::default()
                    }
                    .into(),
                );
            }
        }
        items.push(MenuItem::Separator);

        let has_addr = !self.cfg.get("INDEX01_ADDRESS").is_empty();
        items.push(
            CheckmarkItem {
                label: "Listen to ring".into(),
                checked: self.running(),
                enabled: has_addr,
                activate: Box::new(|this: &mut Self| {
                    let on = !this.running();
                    if let Err(e) = service::set_enabled(on) {
                        this.error = Some(format!("service: {e}"));
                    }
                    let s = Snapshot::read();
                    this.apply(s);
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            CheckmarkItem {
                label: "Paste into focused window".into(),
                checked: self.cfg.flag("INDEX01_PASTE"),
                activate: Box::new(|this: &mut Self| {
                    let on = !this.cfg.flag("INDEX01_PASTE");
                    this.update_cfg(|c| c.set_flag("INDEX01_PASTE", on));
                }),
                ..Default::default()
            }
            .into(),
        );

        // Model: only for the in-process backend.
        if self.cfg.get("INDEX01_BACKEND") == "local" || self.cfg.get("INDEX01_BACKEND").is_empty()
        {
            let mut specs = vec!["auto".to_owned()];
            specs.extend(self.models.iter().cloned());
            let current = self.cfg.get("INDEX01_MODEL_DIR");
            let current = if current.is_empty() { "auto" } else { current };
            let selected = specs.iter().position(|s| s == current).unwrap_or(0);
            let options = specs
                .iter()
                .map(|s| RadioItem {
                    label: if s == "auto" {
                        "Automatic (Parakeet v2 first)".into()
                    } else {
                        s.clone()
                    },
                    ..Default::default()
                })
                .collect();
            items.push(
                SubMenu {
                    label: "Model".into(),
                    submenu: vec![
                        RadioGroup {
                            selected,
                            select: Box::new(move |this: &mut Self, i| {
                                let spec = specs[i].clone();
                                this.update_cfg(|c| c.set("INDEX01_MODEL_DIR", &spec));
                            }),
                            options,
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
            );
        }

        items.extend([
            MenuItem::Separator,
            StandardItem {
                label: "Settings…".into(),
                icon_name: "preferences-system-symbolic".into(),
                activate: Box::new(|_: &mut Self| open_settings()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open recordings folder".into(),
                icon_name: "folder-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let out = std::path::PathBuf::from(this.cfg.get("INDEX01_OUT"));
                    let _ = std::fs::create_dir_all(&out);
                    service::open(&out);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit tray icon".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]);
        items
    }
}
