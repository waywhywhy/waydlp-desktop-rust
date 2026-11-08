//! Installation logic shared by `waydlp-installer` (CLI) and the graphical
//! installer.
//!
//! Principles:
//! - Package managers own what they install. For `.deb`, `.rpm`, Arch
//!   packages and Flatpak the installer only downloads and verifies the file
//!   and then hands it to the system package manager; it never copies files
//!   into package-managed locations itself.
//! - User-level installs (Windows per-user, Linux `~/.local`) need no
//!   administrator rights and record every file they create in a receipt so
//!   uninstall removes exactly those files.
//! - Every downloaded artifact is verified against `SHA256SUMS.txt` from the
//!   same GitHub release before it is used. No remote scripts are executed.

pub mod archive;
pub mod desktop;
pub mod payload;
pub mod plan;
pub mod receipt;
pub mod release;
pub mod system;

pub use plan::{InstallOptions, Installer, Progress};
pub use receipt::Receipt;

use thiserror::Error;
use waydlp_core::artifacts::{Arch, Os, PackageKind};

pub const APP_ID: &str = "io.github.waywhywhy.waydlp";
pub const REPO: &str = "waywhywhy/waydlp-desktop-rust";

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("This platform ({0}) is not supported by the waydlp installer.")]
    UnsupportedPlatform(String),
    #[error("{0} packages cannot be installed on this system: {1}")]
    UnsupportedMethod(PackageKind, String),
    #[error("Network error: {0}")]
    Http(String),
    #[error("No published release was found{0}.")]
    NoRelease(String),
    #[error("The release does not contain {0}.")]
    MissingAsset(String),
    #[error("The release has no checksum for {0}; refusing to install an unverified file.")]
    MissingChecksum(String),
    #[error("Checksum mismatch for {file}: expected {expected}, got {actual}.")]
    ChecksumMismatch { file: String, expected: String, actual: String },
    #[error("Archive error: {0}")]
    Archive(String),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} failed with exit code {1}.")]
    CommandFailed(String, i32),
    #[error("waydlp is not installed by this installer (no receipt found at {0}).")]
    NotInstalled(String),
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for InstallError {
    fn from(e: reqwest::Error) -> Self {
        InstallError::Http(e.to_string())
    }
}

/// The machine we are installing on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub os: Os,
    pub arch: Arch,
}

impl Target {
    pub fn detect() -> Result<Self, InstallError> {
        let os = Os::current().ok_or_else(|| InstallError::UnsupportedPlatform(std::env::consts::OS.into()))?;
        let arch = Arch::current().ok_or_else(|| InstallError::UnsupportedPlatform(std::env::consts::ARCH.into()))?;
        Ok(Self { os, arch })
    }

    pub fn describe(&self) -> String {
        format!("{} {}", self.os.as_str(), self.arch.as_str())
    }
}

/// Information about the Linux distribution from `/etc/os-release`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Distro {
    pub id: String,
    pub id_like: Vec<String>,
    pub pretty_name: String,
}

impl Distro {
    pub fn parse(os_release: &str) -> Self {
        let mut d = Distro::default();
        for line in os_release.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim().trim_matches('"').to_string();
            match k.trim() {
                "ID" => d.id = v.to_ascii_lowercase(),
                "ID_LIKE" => d.id_like = v.split_whitespace().map(|s| s.to_ascii_lowercase()).collect(),
                "PRETTY_NAME" => d.pretty_name = v,
                _ => {}
            }
        }
        d
    }

    pub fn detect() -> Option<Self> {
        std::fs::read_to_string("/etc/os-release").ok().map(|s| Self::parse(&s))
    }

    pub fn is_like(&self, family: &str) -> bool {
        self.id == family || self.id_like.iter().any(|x| x == family)
    }

    /// The native package format for this distribution, if any.
    pub fn native_package(&self) -> Option<PackageKind> {
        if self.is_like("arch") {
            Some(PackageKind::ArchPkg)
        } else if self.is_like("debian") || self.is_like("ubuntu") {
            Some(PackageKind::Deb)
        } else if self.is_like("fedora") || self.is_like("rhel") || self.is_like("suse") || self.is_like("opensuse") {
            Some(PackageKind::Rpm)
        } else {
            None
        }
    }
}

/// Which installation methods work on this machine, best first.
pub fn available_methods(target: Target) -> Vec<(PackageKind, Result<(), String>)> {
    match target.os {
        Os::Windows => vec![
            (PackageKind::WindowsPortable, Ok(())),
            (
                PackageKind::Msi,
                if which::which("msiexec").is_ok() { Ok(()) } else { Err("msiexec was not found".into()) },
            ),
        ],
        Os::Linux => {
            let distro = Distro::detect().unwrap_or_default();
            let has = |c: &str| which::which(c).is_ok();
            let mut v = Vec::new();
            if let Some(native) = distro.native_package() {
                v.push((native, system::check_native(native)));
            }
            for k in [
                PackageKind::Flatpak,
                PackageKind::AppImage,
                PackageKind::TarGz,
                PackageKind::Deb,
                PackageKind::Rpm,
                PackageKind::ArchPkg,
            ] {
                if v.iter().any(|(x, _)| *x == k) {
                    continue;
                }
                let r = match k {
                    PackageKind::Flatpak => {
                        if has("flatpak") {
                            Ok(())
                        } else {
                            Err("flatpak is not installed".into())
                        }
                    }
                    PackageKind::AppImage | PackageKind::TarGz => Ok(()),
                    other => {
                        let native = system::check_native(other);
                        match native {
                            Ok(()) => Ok(()),
                            Err(e) => Err(e),
                        }
                    }
                };
                v.push((k, r));
            }
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_os_release() {
        let d = Distro::parse("NAME=\"CachyOS Linux\"\nID=cachyos\nID_LIKE=arch\nPRETTY_NAME=\"CachyOS\"\n");
        assert_eq!(d.id, "cachyos");
        assert!(d.is_like("arch"));
        assert_eq!(d.native_package(), Some(PackageKind::ArchPkg));

        let d = Distro::parse("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n");
        assert_eq!(d.native_package(), Some(PackageKind::Deb));

        let d = Distro::parse("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n");
        assert_eq!(d.native_package(), Some(PackageKind::Rpm));

        let d = Distro::parse("ID=fedora\n");
        assert_eq!(d.native_package(), Some(PackageKind::Rpm));

        let d = Distro::parse("ID=nixos\n");
        assert_eq!(d.native_package(), None);
    }

    #[test]
    fn target_detects() {
        let t = Target::detect();
        if cfg!(any(target_os = "windows", target_os = "linux"))
            && cfg!(any(target_arch = "x86_64", target_arch = "aarch64"))
        {
            assert!(t.is_ok());
        }
    }
}
