//! Install receipts: an exact record of what a user-level install created,
//! so uninstall and verify touch only those files.

use crate::InstallError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use waydlp_core::artifacts::PackageKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub version: String,
    pub method: PackageKind,
    pub install_dir: Option<PathBuf>,
    /// Application files with their checksums (for `verify`).
    pub files: Vec<FileRecord>,
    /// Shortcuts, desktop entries, icons, symlinks.
    pub extra_paths: Vec<PathBuf>,
    /// Windows uninstall registry key (under HKCU).
    pub registry_key: Option<String>,
    /// For system packages / Flatpak: the package or app id.
    pub package: Option<String>,
    /// For MSI: a cached copy of the MSI used for uninstall.
    pub msi_path: Option<PathBuf>,
    pub installed_at: String,
}

/// Where the receipt lives (outside the install directory).
pub fn receipt_dir() -> Result<PathBuf, InstallError> {
    directories::BaseDirs::new()
        .map(|b| b.data_local_dir().join("waydlp-installer"))
        .ok_or_else(|| InstallError::Other("cannot determine the user data directory".into()))
}

pub fn receipt_path() -> Result<PathBuf, InstallError> {
    Ok(receipt_dir()?.join("receipt.json"))
}

impl Receipt {
    pub fn load() -> Result<Option<Receipt>, InstallError> {
        let p = receipt_path()?;
        match std::fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| InstallError::Other(format!("invalid receipt {}: {e}", p.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<(), InstallError> {
        let p = receipt_path()?;
        std::fs::create_dir_all(p.parent().expect("has parent"))?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(|e| InstallError::Other(e.to_string()))?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }

    pub fn delete() -> Result<(), InstallError> {
        match std::fs::remove_file(receipt_path()?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
