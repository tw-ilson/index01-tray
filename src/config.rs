//! `~/.config/index01/watch.env`: the systemd `EnvironmentFile` that configures `index01 watch`.
//! Every `watch` option has an `INDEX01_*` variable. Unknown keys are preserved on save.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/".into())
}

pub fn env_path() -> PathBuf {
    config_dir().join("index01/watch.env")
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Config(pub BTreeMap<String, String>);

impl Config {
    pub fn defaults() -> Self {
        let mut c = Config::default();
        c.set("INDEX01_OUT", home().join("Index01").to_string_lossy());
        c.set("INDEX01_PASTE", "true");
        c.set("INDEX01_PASTE_KEYS", "ctrl-v");
        c.set("INDEX01_BACKEND", "local");
        c.set("INDEX01_MODEL_DIR", "auto");
        c
    }

    pub fn load() -> Self {
        match std::fs::read_to_string(env_path()) {
            Ok(s) => Self::parse(&s),
            Err(_) => Self::defaults(),
        }
    }

    pub fn parse(s: &str) -> Self {
        let mut m = BTreeMap::new();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                m.insert(k.trim().to_owned(), unquote(v.trim()));
            }
        }
        Config(m)
    }

    pub fn render(&self) -> String {
        let mut s = String::from(
            "# index01 watch settings (written by index01-tray). One INDEX01_* per option.\n",
        );
        for (k, v) in &self.0 {
            if !v.is_empty() {
                s.push_str(&format!("{k}={}\n", quote(v)));
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let p = env_path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, self.render())
    }

    pub fn get(&self, k: &str) -> &str {
        self.0.get(k).map(String::as_str).unwrap_or("")
    }

    pub fn set(&mut self, k: &str, v: impl AsRef<str>) {
        self.0.insert(k.to_owned(), v.as_ref().to_owned());
    }

    pub fn flag(&self, k: &str) -> bool {
        matches!(
            self.get(k).to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    }

    pub fn set_flag(&mut self, k: &str, on: bool) {
        self.set(k, if on { "true" } else { "false" });
    }
}

/// systemd EnvironmentFile quoting: double quotes, backslash escapes.
fn quote(v: &str) -> String {
    if v.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:@+,".contains(c))
    {
        v.to_owned()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn unquote(v: &str) -> String {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        let inner = &v[1..v.len() - 1];
        let mut out = String::new();
        let mut it = inner.chars();
        while let Some(c) = it.next() {
            if c == '\\' && b[0] == b'"' {
                if let Some(n) = it.next() {
                    out.push(n);
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        v.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut c = Config::default();
        c.set("INDEX01_EXEC", r#"notify-send "ring" "$INDEX01_TEXT" \o/"#);
        c.set("INDEX01_ADDRESS", "EA:27:B7:1B:74:51");
        c.set("INDEX01_OUT", "/home/x/My Notes");
        let back = Config::parse(&c.render());
        assert_eq!(back, c);
        assert!(c.render().contains("INDEX01_ADDRESS=EA:27:B7:1B:74:51\n"));
    }
}
