//! The `index01-watch` systemd user service, model discovery, and transcript history.

use crate::config::{config_dir, home};
use std::path::PathBuf;
use std::process::Command;

pub const UNIT: &str = "index01-watch.service";

pub fn unit_path() -> PathBuf {
    config_dir().join("systemd/user").join(UNIT)
}

/// The `index01` CLI: next to this binary, else ~/.local/bin, else $PATH.
pub fn index01_bin() -> PathBuf {
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("index01")));
    [sibling, Some(home().join(".local/bin/index01"))]
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
        .unwrap_or_else(|| "index01".into())
}

pub fn unit_text() -> String {
    format!(
        "[Unit]
Description=Index 01 ring: pull, transcribe and paste recordings
After=bluetooth.target graphical-session.target
PartOf=graphical-session.target

[Service]
ExecStart={bin} watch
EnvironmentFile=%h/.config/index01/watch.env
WorkingDirectory=%h
Restart=on-failure
RestartSec=5

[Install]
WantedBy=graphical-session.target
",
        bin = index01_bin().display()
    )
}

fn systemctl(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("systemctl").arg("--user").args(args).output()
}

/// Write the unit file (if changed) and reload systemd.
pub fn install_unit() -> std::io::Result<()> {
    let p = unit_path();
    let text = unit_text();
    if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, text)?;
        systemctl(&["daemon-reload"])?;
    }
    Ok(())
}

pub fn set_enabled(on: bool) -> std::io::Result<()> {
    install_unit()?;
    let out = if on {
        systemctl(&["enable", "--now", UNIT])?
    } else {
        systemctl(&["disable", "--now", UNIT])?
    };
    if !out.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    Ok(())
}

/// Stop the service for this session (it starts again at next login if enabled).
pub fn stop() {
    let _ = systemctl(&["stop", UNIT]);
}

/// Restart if running, so new settings take effect.
pub fn reload_if_active() -> std::io::Result<()> {
    if state() == "active" {
        install_unit()?;
        systemctl(&["restart", UNIT])?;
    }
    Ok(())
}

/// systemd ActiveState: active, inactive, failed, activating, ...
pub fn state() -> String {
    systemctl(&["is-active", UNIT])
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|_| "unknown".into())
}

/// Last few transcripts (newest last) and the last error line from the service log.
pub fn recent_log() -> (Vec<String>, Option<String>) {
    let Ok(o) = Command::new("journalctl")
        .args(["--user", "-u", UNIT, "-o", "cat", "-n", "300", "--no-pager"])
        .output()
    else {
        return (vec![], None);
    };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut transcripts = vec![];
    let mut error = None;
    for line in text.lines() {
        if let Some(t) = line.strip_prefix(">> ") {
            if !t.trim().is_empty() {
                transcripts.push(t.trim().to_owned());
            }
            error = None;
        } else if line.starts_with("watching ") {
            error = None;
        } else if line.contains("failed") || line.starts_with("Error") || line.starts_with("error")
        {
            error = Some(line.trim().to_owned());
        }
    }
    let n = transcripts.len();
    (transcripts.split_off(n.saturating_sub(5)), error)
}

/// Model folders downloaded by Handy and Spokenly, as `--model-dir` specs (`handy/<folder>`).
pub fn local_models() -> Vec<String> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"));
    let mut v = vec![];
    for (app, id) in [
        ("handy", "com.pais.handy"),
        ("spokenly", "app.spokenly.linux"),
    ] {
        if let Ok(rd) = std::fs::read_dir(data.join(id).join("models")) {
            let mut names: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| format!("{app}/{}", e.file_name().to_string_lossy()))
                .collect();
            names.sort();
            v.extend(names);
        }
    }
    v
}

/// Index 01 rings seen by `index01 scan` as (address, label).
pub fn scan_rings(seconds: u32) -> Result<Vec<(String, String)>, String> {
    let o = Command::new(index01_bin())
        .args(["scan", "--seconds", &seconds.to_string()])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let addr = it.next()?;
            (addr.len() == 17 && addr.chars().filter(|&c| c == ':').count() == 5)
                .then(|| (addr.to_owned(), l.to_owned()))
        })
        .collect())
}

pub fn open(path: &std::path::Path) {
    let _ = Command::new("xdg-open").arg(path).spawn();
}

pub fn copy(text: &str) {
    use std::io::Write;
    if let Ok(mut c) = Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(mut s) = c.stdin.take() {
            let _ = s.write_all(text.as_bytes());
        }
        let _ = c.wait();
    }
}
