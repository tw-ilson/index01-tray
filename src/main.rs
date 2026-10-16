//! index01-tray: tray icon and settings window for `index01 watch` (Index 01 ring transcription).
//!
//!   index01-tray            run the tray icon (left click: settings; right click: menu)
//!   index01-tray settings   open the settings window
//!   index01-tray install    write the systemd unit, default settings, autostart and menu entries
//!   index01-tray uninstall  stop the service and remove those files (settings are kept)

mod config;
mod service;
mod settings;
mod style;
mod tray;

use config::{Config, config_dir, home};
use ksni::blocking::TrayMethods;
use std::time::Duration;

fn desktop_entry(autostart: bool) -> String {
    let exe = std::env::current_exe().unwrap_or_else(|_| "index01-tray".into());
    let (exec, name) = if autostart {
        (format!("{}", exe.display()), "Index 01 tray")
    } else {
        (format!("{} settings", exe.display()), "Index 01")
    };
    format!(
        "[Desktop Entry]
Type=Application
Name={name}
Comment=Transcribe Index 01 ring recordings
Exec={exec}
Icon=audio-input-microphone
Terminal=false
Categories=Utility;Audio;
StartupWMClass=index01-tray
{extra}",
        extra = if autostart {
            "X-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
        } else {
            ""
        }
    )
}

fn autostart_path() -> std::path::PathBuf {
    config_dir().join("autostart/index01-tray.desktop")
}

fn app_entry_path() -> std::path::PathBuf {
    home().join(".local/share/applications/index01-tray.desktop")
}

fn install() -> Result<(), Box<dyn std::error::Error>> {
    if !config::env_path().exists() {
        Config::defaults().save()?;
        println!("wrote {}", config::env_path().display());
    }
    service::install_unit()?;
    println!(
        "wrote {} (runs {})",
        service::unit_path().display(),
        service::index01_bin().display()
    );
    for (p, auto) in [(autostart_path(), true), (app_entry_path(), false)] {
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, desktop_entry(auto))?;
        println!("wrote {}", p.display());
    }
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .contains("GNOME")
    {
        let ok = std::process::Command::new("gnome-extensions")
            .args(["enable", "appindicatorsupport@rgcjonas.gmail.com"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        println!(
            "{}",
            if ok {
                "enabled the GNOME AppIndicator extension (needed for tray icons)"
            } else {
                "GNOME needs the AppIndicator extension for tray icons: install gnome-shell-extension-appindicator"
            }
        );
    }
    if Config::load().get("INDEX01_ADDRESS").is_empty() {
        println!("next: run `index01-tray`, open Settings, scan for the ring, save, then Start");
    } else {
        service::set_enabled(true)?;
        println!("service enabled and started");
    }
    Ok(())
}

fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let _ = service::set_enabled(false);
    for p in [service::unit_path(), autostart_path(), app_entry_path()] {
        if std::fs::remove_file(&p).is_ok() {
            println!("removed {}", p.display());
        }
    }
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    println!("kept settings in {}", config::env_path().display());
    Ok(())
}

fn run_tray() -> Result<(), Box<dyn std::error::Error>> {
    let tray = tray::RingTray::new();
    let mut last = tray.snapshot();
    let handle = tray.spawn()?;
    loop {
        std::thread::sleep(Duration::from_secs(3));
        // Poll outside the tray lock; update() re-sends properties, so only call it on change.
        let fresh = tray::Snapshot::read();
        if fresh != last {
            last = fresh.clone();
            if handle.update(|t| t.apply(fresh)).is_none() {
                return Err("tray service stopped".into());
            }
        }
    }
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let res: Result<(), Box<dyn std::error::Error>> = match cmd.as_str() {
        "" | "tray" => run_tray(),
        "settings" => settings::run().map_err(|e| e.to_string().into()),
        "install" => install(),
        "uninstall" => uninstall(),
        _ => {
            eprintln!("usage: index01-tray [tray | settings | install | uninstall]");
            std::process::exit(2);
        }
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
