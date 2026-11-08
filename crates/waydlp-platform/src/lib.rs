//! Platform integration: application directories (including portable mode),
//! detection of how waydlp was installed, opening files/folders, logging.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File placed next to the executable to enable portable mode.
pub const PORTABLE_MARKER: &str = "waydlp.portable";
/// File placed next to the executable by installers/packages, containing the
/// install kind id (see [`InstallKind::id`]).
pub const INSTALL_KIND_MARKER: &str = "install-kind";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub portable: bool,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
    /// Managed yt-dlp / FFmpeg copies.
    pub bin_dir: PathBuf,
    /// Per-job temporary directories.
    pub jobs_temp_dir: PathBuf,
}

impl AppPaths {
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }
    pub fn history_db(&self) -> PathBuf {
        self.data_dir.join("history.sqlite")
    }
    pub fn archive_file(&self) -> PathBuf {
        self.data_dir.join("download-archive.txt")
    }

    /// Resolve directories. Portable mode is enabled when a
    /// `waydlp.portable` file exists next to the executable; everything is
    /// then kept in a `data` folder beside it and nothing is written elsewhere.
    pub fn resolve() -> Self {
        if let Some(dir) = exe_dir()
            && dir.join(PORTABLE_MARKER).exists()
        {
            return Self::portable_in(&dir.join("data"));
        }
        if let Some(p) = directories::ProjectDirs::from("io.github", "waywhywhy", "waydlp") {
            let data = p.data_local_dir().to_path_buf();
            return Self {
                portable: false,
                config_dir: p.config_dir().to_path_buf(),
                log_dir: data.join("logs"),
                bin_dir: data.join("bin"),
                cache_dir: p.cache_dir().to_path_buf(),
                jobs_temp_dir: p.cache_dir().join("jobs"),
                data_dir: data,
            };
        }
        // Last resort: a folder in the temp directory.
        Self::portable_in(&std::env::temp_dir().join("waydlp"))
    }

    pub fn portable_in(root: &Path) -> Self {
        Self {
            portable: true,
            config_dir: root.join("config"),
            data_dir: root.to_path_buf(),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            bin_dir: root.join("bin"),
            jobs_temp_dir: root.join("cache").join("jobs"),
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for d in [&self.config_dir, &self.data_dir, &self.cache_dir, &self.log_dir, &self.bin_dir] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }
}

/// Directory of the running executable. `canonicalize` is deliberately not
/// used: on Windows it returns verbatim (`\\?\C:\...`) paths, which some programs
/// (including PyInstaller-built yt-dlp) cannot be launched from.
pub fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // Resolve symlinks (e.g. /usr/bin/waydlp -> /opt/waydlp/waydlp) on Unix only.
    #[cfg(unix)]
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent().map(Path::to_path_buf)
}

/// The OS downloads folder, with a `waydlp` sub-folder.
pub fn default_download_dir() -> PathBuf {
    let base = directories::UserDirs::new()
        .and_then(|u| u.download_dir().map(Path::to_path_buf))
        .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("waydlp")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallKind {
    Portable,
    WindowsInstaller,
    Msi,
    AppImage,
    Flatpak,
    Deb,
    Rpm,
    Arch,
    /// Installed to the user's home by `waydlp-installer` from the tarball.
    UserTarball,
    /// Running from a source checkout (`cargo run`).
    Source,
    Unknown,
}

impl InstallKind {
    pub fn id(self) -> &'static str {
        match self {
            InstallKind::Portable => "portable",
            InstallKind::WindowsInstaller => "installer",
            InstallKind::Msi => "msi",
            InstallKind::AppImage => "appimage",
            InstallKind::Flatpak => "flatpak",
            InstallKind::Deb => "deb",
            InstallKind::Rpm => "rpm",
            InstallKind::Arch => "arch",
            InstallKind::UserTarball => "tarball",
            InstallKind::Source => "source",
            InstallKind::Unknown => "unknown",
        }
    }

    pub fn from_id(s: &str) -> Self {
        match s.trim() {
            "portable" => InstallKind::Portable,
            "installer" => InstallKind::WindowsInstaller,
            "msi" => InstallKind::Msi,
            "appimage" => InstallKind::AppImage,
            "flatpak" => InstallKind::Flatpak,
            "deb" => InstallKind::Deb,
            "rpm" => InstallKind::Rpm,
            "arch" => InstallKind::Arch,
            "tarball" => InstallKind::UserTarball,
            "source" => InstallKind::Source,
            _ => InstallKind::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            InstallKind::Portable => "Portable",
            InstallKind::WindowsInstaller => "Windows installer",
            InstallKind::Msi => "MSI package",
            InstallKind::AppImage => "AppImage",
            InstallKind::Flatpak => "Flatpak",
            InstallKind::Deb => "Debian package",
            InstallKind::Rpm => "RPM package",
            InstallKind::Arch => "Arch Linux package",
            InstallKind::UserTarball => "User install (tar.gz)",
            InstallKind::Source => "Built from source",
            InstallKind::Unknown => "Unknown",
        }
    }

    /// How the user should update this installation. waydlp never modifies
    /// files owned by a package manager.
    pub fn update_advice(self) -> &'static str {
        match self {
            InstallKind::Portable => {
                "Download the new portable archive from the releases page and replace this folder (your data folder can be kept)."
            }
            InstallKind::WindowsInstaller => {
                "Download and run the new waydlp setup .exe; it upgrades this installation in place."
            }
            InstallKind::Msi => "Install the new MSI; Windows Installer upgrades the existing version.",
            InstallKind::AppImage => "Download the new AppImage and replace this file.",
            InstallKind::Flatpak => "Run: flatpak update io.github.waywhywhy.waydlp",
            InstallKind::Deb => {
                "Update through your package manager, or install the new .deb with: sudo apt install ./waydlp-<version>-linux-x64.deb"
            }
            InstallKind::Rpm => {
                "Update through your package manager, or install the new .rpm with: sudo dnf install ./waydlp-<version>-linux-x64.rpm"
            }
            InstallKind::Arch => "Update with your AUR helper or pacman (e.g. `yay -Syu` or rebuild with makepkg).",
            InstallKind::UserTarball => "Run: waydlp-installer install --format tar",
            InstallKind::Source => "Pull the latest source and rebuild with cargo.",
            InstallKind::Unknown => "Download the latest release from the waydlp releases page.",
        }
    }
}

/// Detect how the running waydlp was installed.
pub fn detect_install_kind() -> InstallKind {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return InstallKind::Flatpak;
    }
    if std::env::var_os("APPIMAGE").is_some() {
        return InstallKind::AppImage;
    }
    let Some(dir) = exe_dir() else { return InstallKind::Unknown };
    detect_install_kind_in(&dir)
}

pub fn detect_install_kind_in(dir: &Path) -> InstallKind {
    if dir.join(PORTABLE_MARKER).exists() {
        return InstallKind::Portable;
    }
    if let Ok(s) = std::fs::read_to_string(dir.join(INSTALL_KIND_MARKER)) {
        let k = InstallKind::from_id(&s);
        if k != InstallKind::Unknown {
            return k;
        }
    }
    let s = dir.to_string_lossy().replace('\\', "/");
    if s.contains("/target/debug") || s.contains("/target/release") {
        return InstallKind::Source;
    }
    // Packages ship /usr/lib/waydlp/install-kind; if it's missing we still
    // know a system path is package-managed and must not be self-updated.
    if s.starts_with("/usr/") {
        for (marker, kind) in [
            ("/usr/lib/waydlp/install-kind", None),
            ("/var/lib/pacman", Some(InstallKind::Arch)),
            ("/var/lib/dpkg", Some(InstallKind::Deb)),
            ("/var/lib/rpm", Some(InstallKind::Rpm)),
        ] {
            if let Some(k) = kind
                && Path::new(marker).exists()
            {
                return k;
            }
            if kind.is_none()
                && let Ok(s) = std::fs::read_to_string(marker)
            {
                return InstallKind::from_id(&s);
            }
        }
    }
    InstallKind::Unknown
}

/// Open a file with its default application.
pub fn open_path(p: &Path) -> Result<(), String> {
    opener::open(p).map_err(|e| e.to_string())
}

/// Show a file in its folder (selecting it where the OS supports that).
pub fn reveal_path(p: &Path) -> Result<(), String> {
    if p.exists() {
        opener::reveal(p).map_err(|e| e.to_string())
    } else if let Some(parent) = p.parent().filter(|d| d.exists()) {
        opener::open(parent).map_err(|e| e.to_string())
    } else {
        Err(format!("{} no longer exists", p.display()))
    }
}

pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    opener::open_browser(url).map_err(|e| e.to_string())
}

/// Initialise logging to a daily-rotated file (and stderr in debug builds).
/// Returns a guard that must be kept alive for the process lifetime.
pub fn init_logging(log_dir: &Path, debug: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::prelude::*;
    let _ = std::fs::create_dir_all(log_dir);
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("waydlp")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    let level = if debug { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_env("WAYDLP_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("waydlp={level},waydlp_engine={level},warn")));
    let file_layer = tracing_subscriber::fmt::layer().with_writer(writer).with_ansi(false);
    let registry = tracing_subscriber::registry().with(filter).with(file_layer);
    if cfg!(debug_assertions) {
        let _ = registry.with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr)).try_init();
    } else {
        let _ = registry.try_init();
    }
    Some(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_layout() {
        let p = AppPaths::portable_in(Path::new("/x/data"));
        assert!(p.portable);
        assert_eq!(p.settings_file(), Path::new("/x/data/config/settings.toml"));
        assert_eq!(p.history_db(), Path::new("/x/data/history.sqlite"));
        assert_eq!(p.bin_dir, Path::new("/x/data/bin"));
    }

    #[test]
    fn install_kind_markers() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(detect_install_kind_in(d.path()), InstallKind::Unknown);
        std::fs::write(d.path().join(INSTALL_KIND_MARKER), "msi\n").unwrap();
        assert_eq!(detect_install_kind_in(d.path()), InstallKind::Msi);
        std::fs::write(d.path().join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(detect_install_kind_in(d.path()), InstallKind::Portable);
    }

    #[test]
    fn kind_ids_roundtrip() {
        for k in [
            InstallKind::Portable,
            InstallKind::WindowsInstaller,
            InstallKind::Msi,
            InstallKind::AppImage,
            InstallKind::Flatpak,
            InstallKind::Deb,
            InstallKind::Rpm,
            InstallKind::Arch,
            InstallKind::UserTarball,
            InstallKind::Source,
        ] {
            assert_eq!(InstallKind::from_id(k.id()), k);
            assert!(!k.update_advice().is_empty());
        }
    }

    #[test]
    fn rejects_non_web_urls() {
        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
    }
}
