//! Persistent preferences (%APPDATA%\MailCraft\settings.json).

use crate::theme::{Accent, ThemePreference};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: crate::i18n::LangPref,
    pub theme: ThemePreference,
    pub accent: Accent,
    /// Files and folders, most recent first.
    pub recent: Vec<String>,
    /// Download remote pictures without asking (Outlook blocks them by default).
    pub remote_images: bool,
    pub list_width: f32,
    /// Show the conversation preview line in the message list.
    pub preview_lines: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { lang: crate::i18n::LangPref::System, theme: ThemePreference::System, accent: Accent::Blue, recent: Vec::new(), remote_images: false, list_width: 380.0, preview_lines: true }
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(d) = crate::platform::portable_dir() {
        return d;
    }
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(crate::branding::CONFIG_DIR)
}

impl Settings {
    fn file() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(config_dir());
        if let Ok(s) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::file(), s);
        }
    }

    pub fn push_recent(&mut self, p: &Path) {
        let s = p.display().to_string();
        self.recent.retain(|r| !r.eq_ignore_ascii_case(&s));
        self.recent.insert(0, s);
        self.recent.truncate(12);
    }
}
