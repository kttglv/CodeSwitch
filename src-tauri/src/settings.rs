use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub launch_at_login: bool,
    pub undo_shortcut: String,
    pub excluded_apps: Vec<String>,
    pub allowed_apps: Vec<String>,
    pub preview_consent: bool,
    pub onboarding_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            launch_at_login: false,
            undo_shortcut: if cfg!(target_os = "macos") {
                "Meta+Alt+Backspace"
            } else {
                "Ctrl+Alt+Backspace"
            }
            .into(),
            excluded_apps: Vec::new(),
            allowed_apps: Vec::new(),
            preview_consent: false,
            onboarding_complete: false,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Settings could not be read. Correction has been paused; save settings to reset them.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => match std::fs::read(path.with_extension("backup")) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Settings backup could not be read. Correction is paused.".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
                Err(_) => Err("Settings backup could not be opened. Correction is paused.".into()),
            },
            Err(_) => Err("Settings could not be opened. Check the app configuration directory permissions.".into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.excluded_apps.len() + self.allowed_apps.len() > 256
            || self
                .excluded_apps
                .iter()
                .chain(&self.allowed_apps)
                .any(|app| app.is_empty() || app.len() > 500 || app.contains(['\n', '\r', '\0']))
        {
            return Err("Use up to 256 valid app identifiers".into());
        }
        let parent = path.parent().ok_or("Invalid settings directory")?;
        std::fs::create_dir_all(parent).map_err(|_| "Cannot create the settings directory")?;
        let temporary = path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(self).map_err(|_| "Cannot serialize settings")?;
        std::fs::write(&temporary, data).map_err(|_| "Cannot write settings")?;
        // Windows rename does not replace an existing destination. Keep a backup
        // until the new file has been installed, so a failed save retains settings.
        let backup = path.with_extension("backup");
        if path.exists() {
            if backup.exists() {
                std::fs::remove_file(&backup).map_err(|_| "Cannot replace the settings backup")?;
            }
            std::fs::rename(path, &backup).map_err(|_| "Cannot back up settings")?;
        }
        if std::fs::rename(&temporary, path).is_err() {
            let _ = std::fs::rename(&backup, path);
            return Err("Cannot install the new settings".into());
        }
        if backup.exists() {
            let _ = std::fs::remove_file(backup);
        }
        Ok(())
    }

    pub fn excluded(&self, app: &str) -> bool {
        let app = app.to_lowercase();
        if self
            .excluded_apps
            .iter()
            .any(|entry| entry.to_lowercase() == app)
        {
            return true;
        }
        if self
            .allowed_apps
            .iter()
            .any(|entry| entry.to_lowercase() == app)
        {
            return false;
        }
        app.starts_with("com.jetbrains.")
            || DEFAULT_EXCLUSIONS
                .iter()
                .any(|entry| app == *entry || app.ends_with(&format!(".{entry}")))
    }
}

pub const DEFAULT_EXCLUSIONS: &[&str] = &[
    "com.apple.terminal",
    "com.googlecode.iterm2",
    "dev.warp.warp-stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "org.alacritty.alacritty",
    "com.microsoft.vscode",
    "com.microsoft.vscodeinsiders",
    "com.todesktop.230313mzl4w4u92",
    "dev.zed.zed",
    "com.apple.dt.xcode",
    "com.jetbrains.intellij",
    "com.jetbrains.pycharm",
    "com.jetbrains.webstorm",
    "com.jetbrains.rustrover",
    "com.jetbrains.clion",
    "com.sublimetext.4",
    "com.sublimetext.3",
    "com.visualstudio.code.oss",
    "org.gnu.emacs",
    "org.vim.macvim",
    "com.macromates.textmate",
    "windowsterminal.exe",
    "openconsole.exe",
    "conhost.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "code.exe",
    "code - insiders.exe",
    "cursor.exe",
    "windsurf.exe",
    "zed.exe",
    "devenv.exe",
    "idea64.exe",
    "pycharm64.exe",
    "webstorm64.exe",
    "rustrover64.exe",
    "clion64.exe",
    "sublime_text.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "mintty.exe",
    "putty.exe",
    "notepad++.exe",
    "nvim.exe",
    "gvim.exe",
    "vim.exe",
    "emacs.exe",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusions_are_case_insensitive_and_explicit_overrides_win() {
        let mut s = Settings::default();
        assert!(s.excluded("Code.exe"));
        assert!(s.excluded("com.apple.Terminal"));
        assert!(!s.excluded("com.apple.TextEdit"));
        s.allowed_apps.push("Code.exe".into());
        assert!(!s.excluded("code.exe"));
        s.excluded_apps.push("code.exe".into());
        assert!(s.excluded("code.exe"));
    }
}
