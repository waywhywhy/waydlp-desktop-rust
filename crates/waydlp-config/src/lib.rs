//! waydlp settings.
//!
//! Settings are stored as TOML with an explicit `schema_version`. Older
//! files are migrated step by step on load; unknown fields are ignored and
//! missing fields take their defaults, so a damaged or partial file never
//! prevents the app from starting (the original is backed up instead).
#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use waydlp_core::args::{NetworkOptions, validate_proxy, validate_rate_limit};
use waydlp_core::custom_args::parse_custom_args;
use waydlp_core::options::DownloadOptions;
use waydlp_core::presets::{Preset, builtin_presets};
use waydlp_core::sanitize::{DEFAULT_TEMPLATE, validate_output_template};

/// Bump when the on-disk format changes, and add a migration step.
pub const SCHEMA_VERSION: u32 = 2;

pub const DEFAULT_ACCENT: [u8; 3] = [0x9D, 0x4E, 0xDD];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    #[default]
    Home,
    Downloader,
    Queue,
    History,
    Sites,
    Settings,
    About,
}

impl Page {
    pub const ALL: [Page; 7] =
        [Page::Home, Page::Downloader, Page::Queue, Page::History, Page::Sites, Page::Settings, Page::About];

    pub fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Downloader => "Downloader",
            Page::Queue => "Queue",
            Page::History => "History",
            Page::Sites => "Supported Websites",
            Page::Settings => "Settings",
            Page::About => "About",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// `None` = the OS "Downloads" folder (resolved at runtime).
    pub download_dir: Option<PathBuf>,
    pub landing_page: Page,
    pub notify_on_complete: bool,
    pub notify_on_failure: bool,
    /// Ask before quitting while downloads are running.
    pub confirm_quit_with_active: bool,
    pub open_folder_when_done: bool,
    /// Language tag; UI strings are English-only today but the architecture
    /// keeps strings centralised so translations can be added.
    pub language: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            download_dir: None,
            landing_page: Page::Home,
            notify_on_complete: true,
            notify_on_failure: true,
            confirm_quit_with_active: true,
            open_folder_when_done: false,
            language: "en".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadSettings {
    /// Id of the preset applied by default (quick download, batch, new analyses).
    pub default_preset: String,
    pub filename_template: String,
    pub max_concurrent: usize,
    pub auto_retries: u32,
    pub retries: u32,
    pub fragment_retries: u32,
    /// e.g. `2M`; empty = unlimited.
    pub rate_limit: String,
    pub socket_timeout_secs: u32,
    /// Record downloads in an archive file and skip them next time.
    pub use_download_archive: bool,
    /// Ask yt-dlp to analyse playlists before downloading (recommended).
    pub preview_playlists: bool,
    /// Ask before adding playlists with more items than this.
    pub playlist_confirm_threshold: usize,
    pub playlist_subfolder: bool,
    pub playlist_numbering: bool,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            default_preset: "mp4-1080".into(),
            filename_template: DEFAULT_TEMPLATE.into(),
            max_concurrent: 3,
            auto_retries: 2,
            retries: 10,
            fragment_retries: 10,
            rate_limit: String::new(),
            socket_timeout_secs: 30,
            use_download_archive: false,
            preview_playlists: true,
            playlist_confirm_threshold: 50,
            playlist_subfolder: true,
            playlist_numbering: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub accent: [u8; 3],
    /// 0.75 ..= 2.0
    pub ui_scale: f32,
    pub animations: bool,
    pub reduced_motion: bool,
    pub density: Density,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            accent: DEFAULT_ACCENT,
            ui_scale: 1.0,
            animations: true,
            reduced_motion: false,
            density: Density::Comfortable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AdvancedSettings {
    pub yt_dlp_path: Option<PathBuf>,
    pub ffmpeg_path: Option<PathBuf>,
    pub proxy: String,
    pub custom_args: String,
    /// Netscape-format cookies file the user explicitly imported.
    pub cookies_file: Option<PathBuf>,
    pub use_yt_dlp_config: bool,
    pub debug_logging: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// Keep history for this many days; 0 = forever.
    pub history_retention_days: u32,
    pub save_history: bool,
    /// Opt-in: watch the clipboard for links while waydlp is focused.
    pub clipboard_watch: bool,
    /// Opt-in: check GitHub for new waydlp versions on start-up.
    pub check_updates_on_start: bool,
    /// Load thumbnails from the media site during analysis/history.
    pub load_thumbnails: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            history_retention_days: 0,
            save_history: true,
            clipboard_watch: false,
            check_updates_on_start: false,
            load_thumbnails: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub general: GeneralSettings,
    pub downloads: DownloadSettings,
    pub appearance: AppearanceSettings,
    pub advanced: AdvancedSettings,
    pub privacy: PrivacySettings,
    /// User-defined presets (built-ins are not stored).
    pub presets: Vec<Preset>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            downloads: DownloadSettings::default(),
            appearance: AppearanceSettings::default(),
            advanced: AdvancedSettings::default(),
            privacy: PrivacySettings::default(),
            presets: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not read or write settings: {0}")]
    Io(#[from] std::io::Error),
    #[error("The settings file is not valid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Could not serialise settings: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("This settings file was written by a newer waydlp (schema {0}); please update waydlp.")]
    TooNew(u32),
    #[error("{0}")]
    Invalid(String),
}

impl Settings {
    /// All presets: built-ins first, then user presets.
    pub fn all_presets(&self) -> Vec<Preset> {
        let mut v = builtin_presets();
        v.extend(self.presets.iter().cloned());
        v
    }

    pub fn preset(&self, id: &str) -> Option<Preset> {
        self.all_presets().into_iter().find(|p| p.id == id)
    }

    /// Options of the default preset (falls back to the first built-in).
    pub fn default_options(&self) -> DownloadOptions {
        let mut o = self.preset(&self.downloads.default_preset).unwrap_or_else(|| builtin_presets().remove(0)).options;
        o.playlist.subfolder = self.downloads.playlist_subfolder;
        o.playlist.number_files = self.downloads.playlist_numbering;
        o
    }

    pub fn network_options(&self) -> NetworkOptions {
        let rate = self.downloads.rate_limit.trim();
        let proxy = self.advanced.proxy.trim();
        NetworkOptions {
            rate_limit: (!rate.is_empty()).then(|| rate.to_string()),
            retries: self.downloads.retries,
            fragment_retries: self.downloads.fragment_retries,
            socket_timeout_secs: Some(self.downloads.socket_timeout_secs.max(1)),
            proxy: (!proxy.is_empty()).then(|| proxy.to_string()),
        }
    }

    /// Validate fields that would make downloads fail. Returns readable problems.
    pub fn validate(&self) -> Vec<String> {
        let mut p = Vec::new();
        if let Err(e) = validate_output_template(&self.downloads.filename_template) {
            p.push(format!("Filename template: {e}"));
        }
        let rate = self.downloads.rate_limit.trim();
        if !rate.is_empty()
            && let Err(e) = validate_rate_limit(rate)
        {
            p.push(e.to_string());
        }
        let proxy = self.advanced.proxy.trim();
        if !proxy.is_empty()
            && let Err(e) = validate_proxy(proxy)
        {
            p.push(e.to_string());
        }
        if let Err(e) = parse_custom_args(&self.advanced.custom_args) {
            p.push(format!("Extra yt-dlp arguments: {e}"));
        }
        if self.downloads.max_concurrent == 0 || self.downloads.max_concurrent > 16 {
            p.push("Concurrent downloads must be between 1 and 16.".into());
        }
        p
    }

    /// Clamp numeric values into supported ranges.
    pub fn normalise(&mut self) {
        self.downloads.max_concurrent = self.downloads.max_concurrent.clamp(1, 16);
        self.downloads.auto_retries = self.downloads.auto_retries.min(10);
        self.downloads.retries = self.downloads.retries.min(100);
        self.downloads.fragment_retries = self.downloads.fragment_retries.min(100);
        self.downloads.socket_timeout_secs = self.downloads.socket_timeout_secs.clamp(1, 600);
        self.appearance.ui_scale =
            if self.appearance.ui_scale.is_finite() { self.appearance.ui_scale.clamp(0.75, 2.0) } else { 1.0 };
        if self.downloads.filename_template.trim().is_empty() {
            self.downloads.filename_template = DEFAULT_TEMPLATE.into();
        }
        // User presets must not shadow built-in ids.
        let builtin: Vec<String> = builtin_presets().into_iter().map(|p| p.id).collect();
        self.presets.retain(|p| !builtin.contains(&p.id));
        for p in &mut self.presets {
            p.builtin = false;
        }
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let body = toml::to_string_pretty(self)?;
        Ok(format!("# waydlp settings — edited by the app. Manual edits are allowed while waydlp is closed.\n{body}"))
    }

    /// Parse (and migrate) settings from TOML text.
    pub fn from_toml(text: &str) -> Result<Settings, ConfigError> {
        let mut value: toml::Table = text.parse::<toml::Table>()?;
        let version = value.get("schema_version").and_then(|v| v.as_integer()).unwrap_or(1).max(0) as u32;
        if version > SCHEMA_VERSION {
            return Err(ConfigError::TooNew(version));
        }
        migrate(&mut value, version);
        let mut s: Settings = toml::Value::Table(value).try_into()?;
        s.schema_version = SCHEMA_VERSION;
        s.normalise();
        Ok(s)
    }
}

/// Apply migrations from `from` up to [`SCHEMA_VERSION`] on raw TOML.
fn migrate(t: &mut toml::Table, from: u32) {
    if from < 2 {
        // v1 stored `concurrent_downloads` and `accent_color` ("#RRGGBB") at the top level.
        let mut downloads = t.remove("downloads").and_then(|v| v.as_table().cloned()).unwrap_or_default();
        if let Some(c) = t.remove("concurrent_downloads") {
            downloads.insert("max_concurrent".into(), c);
        }
        if !downloads.is_empty() {
            t.insert("downloads".into(), toml::Value::Table(downloads));
        }
        if let Some(hex) = t.remove("accent_color").and_then(|v| v.as_str().map(str::to_string))
            && let Some(rgb) = parse_hex_color(&hex)
        {
            let mut ap = t.remove("appearance").and_then(|v| v.as_table().cloned()).unwrap_or_default();
            ap.insert(
                "accent".into(),
                toml::Value::Array(rgb.iter().map(|c| toml::Value::Integer(*c as i64)).collect()),
            );
            t.insert("appearance".into(), toml::Value::Table(ap));
        }
        t.insert("schema_version".into(), toml::Value::Integer(2));
    }
}

pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

/// Loads and saves settings at a fixed path.
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load settings. Missing file → defaults. Corrupt file → defaults and
    /// the bad file is preserved as `settings.toml.bak` (never lost).
    pub fn load(&self) -> (Settings, Option<String>) {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => match Settings::from_toml(&text) {
                Ok(s) => (s, None),
                Err(ConfigError::TooNew(v)) => (
                    Settings::default(),
                    Some(format!(
                        "Your settings were saved by a newer waydlp (schema {v}). Defaults are used for this session and the file is left untouched."
                    )),
                ),
                Err(e) => {
                    let bak = self.path.with_extension("toml.bak");
                    let _ = std::fs::copy(&self.path, &bak);
                    tracing::warn!(error = %e, "settings file invalid; using defaults");
                    (
                        Settings::default(),
                        Some(format!(
                            "Your settings file could not be read ({e}). Defaults were loaded; the old file was saved as {}.",
                            bak.display()
                        )),
                    )
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
            Err(e) => (Settings::default(), Some(format!("Could not read settings: {e}"))),
        }
    }

    /// Atomically write settings (write temp file, then rename).
    pub fn save(&self, settings: &Settings) -> Result<(), ConfigError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = settings.to_toml()?;
        let tmp = self.path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Export to an arbitrary file chosen by the user.
    pub fn export(settings: &Settings, to: &Path) -> Result<(), ConfigError> {
        std::fs::write(to, settings.to_toml()?)?;
        Ok(())
    }

    /// Import from a file. Paths to executables and the cookies file are
    /// **not** imported: they are machine-specific and importing an
    /// executable path from someone else's file would be a security risk.
    pub fn import(from: &Path, current: &Settings) -> Result<Settings, ConfigError> {
        let text = std::fs::read_to_string(from)?;
        let mut s = Settings::from_toml(&text)?;
        s.advanced.yt_dlp_path = current.advanced.yt_dlp_path.clone();
        s.advanced.ffmpeg_path = current.advanced.ffmpeg_path.clone();
        s.advanced.cookies_file = current.advanced.cookies_file.clone();
        s.general.download_dir = current.general.download_dir.clone();
        let problems = s.validate();
        if !problems.is_empty() {
            return Err(ConfigError::Invalid(problems.join("\n")));
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waydlp_core::presets::Preset;

    #[test]
    fn defaults_roundtrip() {
        let s = Settings::default();
        let text = s.to_toml().unwrap();
        let back = Settings::from_toml(&text).unwrap();
        assert_eq!(back, s);
        assert!(s.validate().is_empty());
    }

    #[test]
    fn partial_file_uses_defaults() {
        let s = Settings::from_toml("schema_version = 2\n[downloads]\nmax_concurrent = 5\n").unwrap();
        assert_eq!(s.downloads.max_concurrent, 5);
        assert_eq!(s.downloads.retries, DownloadSettings::default().retries);
        assert_eq!(s.appearance.accent, DEFAULT_ACCENT);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let s = Settings::from_toml("schema_version = 2\nfuture_thing = 1\n[general]\nbogus = true\n").unwrap();
        assert_eq!(s.general, GeneralSettings::default());
    }

    #[test]
    fn migrates_v1() {
        let v1 = "concurrent_downloads = 7\naccent_color = \"#FF0080\"\n";
        let s = Settings::from_toml(v1).unwrap();
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.downloads.max_concurrent, 7);
        assert_eq!(s.appearance.accent, [0xFF, 0x00, 0x80]);
    }

    #[test]
    fn rejects_newer_schema() {
        assert!(matches!(Settings::from_toml("schema_version = 99\n"), Err(ConfigError::TooNew(99))));
    }

    #[test]
    fn normalises_ranges() {
        let s =
            Settings::from_toml("schema_version = 2\n[downloads]\nmax_concurrent = 0\n[appearance]\nui_scale = 9.0\n")
                .unwrap();
        assert_eq!(s.downloads.max_concurrent, 1);
        assert_eq!(s.appearance.ui_scale, 2.0);
    }

    #[test]
    fn validation_reports_problems() {
        let mut s = Settings::default();
        s.downloads.filename_template = "../x".into();
        s.downloads.rate_limit = "fast".into();
        s.advanced.proxy = "nope".into();
        s.advanced.custom_args = "--exec calc".into();
        assert_eq!(s.validate().len(), 4);
    }

    #[test]
    fn store_save_load_and_corruption_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.toml"));
        let (s, warn) = store.load();
        assert!(warn.is_none());
        let mut s = s;
        s.downloads.max_concurrent = 4;
        s.presets.push(Preset { id: "user-x".into(), name: "X".into(), builtin: false, options: Default::default() });
        store.save(&s).unwrap();
        let (loaded, warn) = store.load();
        assert!(warn.is_none());
        assert_eq!(loaded, s);
        assert_eq!(loaded.all_presets().last().unwrap().id, "user-x");

        std::fs::write(store.path(), "this is [not toml").unwrap();
        let (loaded, warn) = store.load();
        assert_eq!(loaded, Settings::default());
        assert!(warn.is_some());
        assert!(dir.path().join("settings.toml.bak").exists());
    }

    #[test]
    fn import_keeps_machine_specific_paths() {
        let dir = tempfile::tempdir().unwrap();
        let mut other = Settings::default();
        other.advanced.yt_dlp_path = Some("/evil/yt-dlp".into());
        other.downloads.max_concurrent = 6;
        let f = dir.path().join("export.toml");
        SettingsStore::export(&other, &f).unwrap();
        let mut current = Settings::default();
        current.advanced.yt_dlp_path = Some("/mine/yt-dlp".into());
        let imported = SettingsStore::import(&f, &current).unwrap();
        assert_eq!(imported.downloads.max_concurrent, 6);
        assert_eq!(imported.advanced.yt_dlp_path, Some(PathBuf::from("/mine/yt-dlp")));
    }

    #[test]
    fn user_presets_cannot_shadow_builtins() {
        let mut s = Settings::default();
        s.presets.push(Preset { id: "mp3".into(), name: "Fake".into(), builtin: true, options: Default::default() });
        s.normalise();
        assert!(s.presets.is_empty());
    }

    #[test]
    fn default_options_follow_playlist_settings() {
        let mut s = Settings::default();
        s.downloads.playlist_numbering = false;
        assert!(!s.default_options().playlist.number_files);
        s.downloads.default_preset = "missing".into();
        let _ = s.default_options();
    }

    #[test]
    fn hex_colors() {
        assert_eq!(parse_hex_color("#9D4EDD"), Some([0x9D, 0x4E, 0xDD]));
        assert_eq!(parse_hex_color("zzz"), None);
    }
}
