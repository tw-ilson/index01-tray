# index01-tray

Tray icon and settings window for [index01-rs](https://github.com/tw-ilson/index01-rs): use the
Pebble Index 01 ring on the Linux desktop with no phone and no account. Hold the ring button,
speak, release; the clip is pulled over Bluetooth, transcribed locally, and pasted into the
focused window.

`index01 watch` runs as a systemd user service (`index01-watch.service`). This app:

- **Tray icon** (StatusNotifierItem): shows whether it is listening; the tooltip shows the last
  transcript. Left click opens Settings. Right click menu: status, recent transcripts (click to
  copy), *Listen to ring* on/off, *Paste into focused window*, model choice, open the recordings
  folder.
- **Settings window** (egui): ring address with Bluetooth scan, recordings folder, keep-on-ring,
  transcription backend (local ONNX model, OpenAI-compatible server, or any command), model,
  language, paste shortcut, an extra command per transcript, Start/Stop, recent transcripts.

Settings live in `~/.config/index01/watch.env` as `INDEX01_*` variables, which the service
reads as its `EnvironmentFile`. Saving restarts the service.

## Install

```sh
# the CLI (see index01-rs for ring setup and bonding)
cargo install --locked --git https://github.com/tw-ilson/index01-rs
# this app
cargo install --locked --git https://github.com/tw-ilson/index01-tray
index01-tray install
index01-tray &
```

`install` writes the systemd unit, default settings, a login autostart entry for the tray and an
app-menu entry (*Index 01*, opens Settings). It starts the service if a ring address is set. On
GNOME it enables the AppIndicator extension, which tray icons need (Fedora:
`gnome-shell-extension-appindicator`).

`index01-tray uninstall` stops the service and removes those files; settings are kept.

## Commands

```
index01-tray            tray icon
index01-tray settings   settings window
index01-tray install    unit + autostart + menu entry, start service
index01-tray uninstall
```

The service log is `journalctl --user -u index01-watch -f`.
