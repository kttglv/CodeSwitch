use codeswitch_engine::{word_key, Action, ActionKind, MAX_WORD_CHARS};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Bound on the never-correct list; learning drops the oldest entry beyond it.
pub const MAX_IGNORED_WORDS: usize = 500;

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
    #[serde(default = "default_true")]
    pub automatic_update_checks: bool,
    /// Converts the word before the caret on request. Empty disables it.
    #[serde(default = "default_convert_shortcut")]
    pub convert_shortcut: String,
    /// Words (in either layout's form) that automatic correction leaves alone.
    #[serde(default)]
    pub ignored_words: Vec<String>,
    /// Add a word to `ignored_words` when the user undoes its automatic correction.
    #[serde(default = "default_true")]
    pub learn_from_undo: bool,
    /// Interface language: "system", "en" or "ru".
    #[serde(default = "default_system")]
    pub language: String,
    /// Interface appearance: "system", "light" or "dark".
    #[serde(default = "default_system")]
    pub theme: String,
}

pub const LANGUAGES: &[&str] = &["system", "en", "ru"];
pub const THEMES: &[&str] = &["system", "light", "dark"];

fn default_system() -> String {
    "system".into()
}

fn default_true() -> bool {
    true
}

fn default_convert_shortcut() -> String {
    if cfg!(target_os = "macos") {
        "Ctrl+Meta+Backspace"
    } else {
        "Ctrl+Shift+Backspace"
    }
    .into()
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
            automatic_update_checks: true,
            convert_shortcut: default_convert_shortcut(),
            ignored_words: Vec::new(),
            learn_from_undo: true,
            language: default_system(),
            theme: default_system(),
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
        if self.ignored_words.len() > MAX_IGNORED_WORDS
            || self.ignored_words.iter().any(|word| {
                word.trim().is_empty()
                    || word.chars().count() > MAX_WORD_CHARS
                    || word.chars().any(char::is_control)
            })
        {
            return Err(format!(
                "Use up to {MAX_IGNORED_WORDS} words of up to {MAX_WORD_CHARS} characters"
            ));
        }
        if !LANGUAGES.contains(&self.language.as_str()) || !THEMES.contains(&self.theme.as_str()) {
            return Err("Choose a supported language and appearance".into());
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

    /// Whether either side of an automatic correction is on the never-correct list.
    pub fn ignores(&self, action: &Action) -> bool {
        let original = word_key(&action.expected);
        let replacement = word_key(&action.replacement);
        self.ignored_words.iter().any(|word| {
            let word = word_key(word);
            word == original || word == replacement
        })
    }

    /// After a verified undo of an automatic correction, remember the restored
    /// (typed) word so it is not corrected again. Returns whether the list changed.
    pub fn learn_from(&mut self, action: &Action) -> bool {
        self.learn_from_undo
            && action.kind == (ActionKind::Undo { automatic: true })
            && self.remember_word(&action.replacement)
    }

    /// Add a word to the never-correct list. Returns whether the list changed.
    pub fn remember_word(&mut self, text: &str) -> bool {
        let key = word_key(text);
        if key.is_empty()
            || key.chars().count() > MAX_WORD_CHARS
            || self.ignored_words.iter().any(|word| word_key(word) == key)
        {
            return false;
        }
        if self.ignored_words.len() >= MAX_IGNORED_WORDS {
            self.ignored_words.remove(0);
        }
        self.ignored_words.push(key);
        true
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
    fn existing_settings_keep_preferences_when_updater_is_added() {
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        json.as_object_mut()
            .unwrap()
            .remove("automaticUpdateChecks");
        json["enabled"] = true.into();
        let settings: Settings = serde_json::from_value(json).unwrap();
        assert!(settings.enabled && settings.automatic_update_checks);
        let mut disabled = serde_json::to_value(settings).unwrap();
        disabled["automaticUpdateChecks"] = false.into();
        assert!(
            !serde_json::from_value::<Settings>(disabled)
                .unwrap()
                .automatic_update_checks
        );
    }
    #[test]
    fn existing_settings_gain_word_list_and_convert_defaults() {
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        let object = json.as_object_mut().unwrap();
        for key in ["convertShortcut", "ignoredWords", "learnFromUndo"] {
            object.remove(key);
        }
        object.insert("undoShortcut".into(), "Ctrl+Alt+F9".into());
        let settings: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(settings.undo_shortcut, "Ctrl+Alt+F9");
        assert_eq!(
            settings.convert_shortcut,
            Settings::default().convert_shortcut
        );
        assert!(settings.ignored_words.is_empty() && settings.learn_from_undo);
    }

    #[test]
    fn existing_settings_gain_interface_defaults_and_reject_unknown_values() {
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        let object = json.as_object_mut().unwrap();
        object.remove("language");
        object.remove("theme");
        let settings: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(
            (settings.language.as_str(), settings.theme.as_str()),
            ("system", "system")
        );
        let directory =
            std::env::temp_dir().join(format!("codeswitch-interface-{}", std::process::id()));
        let path = directory.join("settings.json");
        let bad = Settings {
            language: "de".into(),
            ..Settings::default()
        };
        assert!(bad.save(&path).is_err());
        let bad = Settings {
            theme: "blue".into(),
            ..Settings::default()
        };
        assert!(bad.save(&path).is_err());
        let good = Settings {
            language: "ru".into(),
            theme: "dark".into(),
            ..Settings::default()
        };
        assert!(good.save(&path).is_ok());
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(
            (loaded.language.as_str(), loaded.theme.as_str()),
            ("ru", "dark")
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    fn action(expected: &str, replacement: &str) -> Action {
        Action {
            expected: expected.into(),
            replacement: replacement.into(),
            target: codeswitch_engine::Layout::Russian,
            kind: codeswitch_engine::ActionKind::Automatic,
        }
    }

    #[test]
    fn ignored_words_match_either_form_of_a_correction() {
        let mut s = Settings::default();
        assert!(!s.ignores(&action("ghbdtn ", "привет ")));
        s.ignored_words.push("ghbdtn".into());
        assert!(s.ignores(&action("Ghbdtn! ", "Привет! ")));
        s.ignored_words = vec!["Привет".into()];
        assert!(s.ignores(&action("ghbdtn ", "привет ")));
    }

    #[test]
    fn remembering_words_normalizes_deduplicates_and_bounds_the_list() {
        let mut s = Settings::default();
        assert!(s.remember_word("(Ghbdtn) "));
        assert!(!s.remember_word("ghbdtn"), "already present");
        assert!(!s.remember_word(" !? "), "nothing to remember");
        assert_eq!(s.ignored_words, ["ghbdtn"]);
        for i in 0..MAX_IGNORED_WORDS {
            s.remember_word(&format!("word{i}"));
        }
        assert_eq!(s.ignored_words.len(), MAX_IGNORED_WORDS);
        assert!(
            !s.ignored_words.contains(&"ghbdtn".into()),
            "oldest is dropped"
        );
    }

    #[test]
    fn only_undoing_an_automatic_correction_teaches_a_word() {
        use codeswitch_engine::ActionKind;
        let undo = |automatic| Action {
            expected: "привет ".into(),
            replacement: "ghbdtn ".into(),
            target: codeswitch_engine::Layout::English,
            kind: ActionKind::Undo { automatic },
        };
        let mut s = Settings::default();
        assert!(!s.learn_from(&undo(false)), "undoing a manual conversion");
        assert!(!s.learn_from(&action("ghbdtn ", "привет ")), "not an undo");
        s.learn_from_undo = false;
        assert!(!s.learn_from(&undo(true)), "learning is turned off");
        s.learn_from_undo = true;
        assert!(s.learn_from(&undo(true)));
        assert_eq!(s.ignored_words, ["ghbdtn"], "the restored, typed form");
        assert!(s.ignores(&action("ghbdtn ", "привет ")));
    }

    #[test]
    fn saving_rejects_invalid_word_lists() {
        let directory =
            std::env::temp_dir().join(format!("codeswitch-test-{}", std::process::id()));
        let path = directory.join("settings.json");
        let mut s = Settings {
            ignored_words: vec!["fine".into(), "x".repeat(65)],
            ..Settings::default()
        };
        assert!(s.save(&path).is_err());
        s.ignored_words = vec!["two\nlines".into()];
        assert!(s.save(&path).is_err());
        s.ignored_words = vec!["".into()];
        assert!(s.save(&path).is_err());
        s.ignored_words = vec!["fine".into(); MAX_IGNORED_WORDS + 1];
        assert!(s.save(&path).is_err());
        s.ignored_words = vec!["fine".into()];
        assert!(s.save(&path).is_ok());
        assert_eq!(Settings::load(&path).unwrap().ignored_words, ["fine"]);
        let _ = std::fs::remove_dir_all(directory);
    }

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
