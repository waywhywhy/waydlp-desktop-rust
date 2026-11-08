//! Release artifact naming, shared by the release workflow, the installers
//! and (as a documented convention) the website.
//!
//! Pattern: `waydlp-{version}-{os}-{arch}[-{variant}].{ext}`
//!
//! Examples:
//! - `waydlp-1.0.0-windows-x64-setup.exe`
//! - `waydlp-1.0.0-windows-x64.msi`
//! - `waydlp-1.0.0-windows-x64-portable.zip`
//! - `waydlp-1.0.0-linux-x64.AppImage`
//! - `waydlp-1.0.0-linux-x64.deb`
//! - `waydlp-1.0.0-linux-x64.rpm`
//! - `waydlp-1.0.0-linux-x64.tar.gz`
//! - `waydlp-1.0.0-linux-x64.flatpak`
//! - `waydlp-1.0.0-linux-x64.pkg.tar.zst`
//!
//! Every release also carries `SHA256SUMS.txt` in `sha256sum` format.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CHECKSUM_FILE: &str = "SHA256SUMS.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    Arm64,
}

impl Arch {
    pub fn as_str(self) -> &'static str {
        match self {
            Arch::X64 => "x64",
            Arch::Arm64 => "arm64",
        }
    }

    /// Architecture of the running binary.
    pub fn current() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Arch::X64),
            "aarch64" => Some(Arch::Arm64),
            _ => None,
        }
    }
}

impl Os {
    pub fn as_str(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Linux => "linux",
        }
    }

    pub fn current() -> Option<Self> {
        match std::env::consts::OS {
            "windows" => Some(Os::Windows),
            "linux" => Some(Os::Linux),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    WindowsSetup,
    Msi,
    WindowsPortable,
    AppImage,
    Deb,
    Rpm,
    TarGz,
    Flatpak,
    ArchPkg,
}

impl PackageKind {
    pub const ALL: [PackageKind; 9] = [
        PackageKind::WindowsSetup,
        PackageKind::Msi,
        PackageKind::WindowsPortable,
        PackageKind::AppImage,
        PackageKind::Deb,
        PackageKind::Rpm,
        PackageKind::TarGz,
        PackageKind::Flatpak,
        PackageKind::ArchPkg,
    ];

    pub fn os(self) -> Os {
        match self {
            PackageKind::WindowsSetup | PackageKind::Msi | PackageKind::WindowsPortable => Os::Windows,
            _ => Os::Linux,
        }
    }

    /// Suffix appended after `{os}-{arch}`.
    pub fn suffix(self) -> &'static str {
        match self {
            PackageKind::WindowsSetup => "-setup.exe",
            PackageKind::Msi => ".msi",
            PackageKind::WindowsPortable => "-portable.zip",
            PackageKind::AppImage => ".AppImage",
            PackageKind::Deb => ".deb",
            PackageKind::Rpm => ".rpm",
            PackageKind::TarGz => ".tar.gz",
            PackageKind::Flatpak => ".flatpak",
            PackageKind::ArchPkg => ".pkg.tar.zst",
        }
    }

    /// The short id used on the command line (`--format deb`).
    pub fn cli_name(self) -> &'static str {
        match self {
            PackageKind::WindowsSetup => "setup",
            PackageKind::Msi => "msi",
            PackageKind::WindowsPortable => "portable",
            PackageKind::AppImage => "appimage",
            PackageKind::Deb => "deb",
            PackageKind::Rpm => "rpm",
            PackageKind::TarGz => "tar",
            PackageKind::Flatpak => "flatpak",
            PackageKind::ArchPkg => "arch",
        }
    }

    pub fn from_cli_name(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        Self::ALL.into_iter().find(|k| k.cli_name() == s).or(match s.as_str() {
            "exe" => Some(PackageKind::WindowsSetup),
            "zip" => Some(PackageKind::WindowsPortable),
            "tar.gz" | "tarball" | "targz" => Some(PackageKind::TarGz),
            "pacman" | "aur" | "pkgbuild" => Some(PackageKind::ArchPkg),
            _ => None,
        })
    }

    pub fn display_name(self) -> &'static str {
        match self {
            PackageKind::WindowsSetup => "Windows installer (.exe)",
            PackageKind::Msi => "Windows MSI package",
            PackageKind::WindowsPortable => "Windows portable (.zip)",
            PackageKind::AppImage => "AppImage",
            PackageKind::Deb => "Debian / Ubuntu package (.deb)",
            PackageKind::Rpm => "Fedora / openSUSE package (.rpm)",
            PackageKind::TarGz => "Portable archive (.tar.gz)",
            PackageKind::Flatpak => "Flatpak bundle",
            PackageKind::ArchPkg => "Arch Linux package",
        }
    }
}

impl fmt::Display for PackageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.cli_name())
    }
}

/// Build the canonical file name.
pub fn artifact_name(version: &str, kind: PackageKind, arch: Arch) -> String {
    let v = version.trim_start_matches('v');
    format!("waydlp-{v}-{}-{}{}", kind.os().as_str(), arch.as_str(), kind.suffix())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArtifact {
    pub version: String,
    pub kind: PackageKind,
    pub arch: Arch,
}

/// Parse a file name produced by [`artifact_name`].
pub fn parse_artifact_name(name: &str) -> Option<ParsedArtifact> {
    let rest = name.strip_prefix("waydlp-")?;
    for kind in PackageKind::ALL {
        let Some(stem) = rest.strip_suffix(kind.suffix()) else { continue };
        for arch in [Arch::X64, Arch::Arm64] {
            let tail = format!("-{}-{}", kind.os().as_str(), arch.as_str());
            if let Some(version) = stem.strip_suffix(&tail)
                && !version.is_empty()
                && version.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                return Some(ParsedArtifact { version: version.to_string(), kind, arch });
            }
        }
    }
    None
}

/// Parse a `sha256sum`-style file into `(file_name, lowercase_hex)` pairs.
pub fn parse_checksums(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let (hash, name) = l.split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*').trim();
            if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) && !name.is_empty() {
                Some((name.to_string(), hash.to_ascii_lowercase()))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        for kind in PackageKind::ALL {
            for arch in [Arch::X64, Arch::Arm64] {
                let n = artifact_name("v1.2.3", kind, arch);
                assert!(n.starts_with("waydlp-1.2.3-"), "{n}");
                let p = parse_artifact_name(&n).unwrap();
                assert_eq!(p, ParsedArtifact { version: "1.2.3".into(), kind, arch });
            }
        }
        assert_eq!(artifact_name("1.0.0", PackageKind::WindowsSetup, Arch::X64), "waydlp-1.0.0-windows-x64-setup.exe");
        assert_eq!(artifact_name("1.0.0-rc.1", PackageKind::Deb, Arch::X64), "waydlp-1.0.0-rc.1-linux-x64.deb");
        assert_eq!(parse_artifact_name("waydlp-1.0.0-rc.1-linux-x64.deb").unwrap().version, "1.0.0-rc.1");
    }

    #[test]
    fn rejects_foreign_names() {
        assert!(parse_artifact_name("SHA256SUMS.txt").is_none());
        assert!(parse_artifact_name("waydlp-linux-x64.deb").is_none());
        assert!(parse_artifact_name("other-1.0.0-linux-x64.deb").is_none());
    }

    #[test]
    fn checksums() {
        let h = "a".repeat(64);
        let text = format!("{h}  waydlp-1.0.0-linux-x64.deb\n{h} *waydlp.exe\n# comment\nbad line\n");
        let c = parse_checksums(&text);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].0, "waydlp-1.0.0-linux-x64.deb");
        assert_eq!(c[1].0, "waydlp.exe");
    }

    #[test]
    fn cli_names() {
        assert_eq!(PackageKind::from_cli_name("AppImage"), Some(PackageKind::AppImage));
        assert_eq!(PackageKind::from_cli_name("aur"), Some(PackageKind::ArchPkg));
        assert_eq!(PackageKind::from_cli_name("nope"), None);
    }
}
