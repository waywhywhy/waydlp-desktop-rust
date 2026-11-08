//! Fetching release metadata and verified artifacts from GitHub Releases.

use crate::{InstallError, REPO};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use waydlp_core::artifacts::{Arch, CHECKSUM_FILE, PackageKind, artifact_name, parse_checksums};

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub assets: Vec<Asset>,
    pub checksums: HashMap<String, String>,
}

impl Release {
    pub fn asset_for(&self, kind: PackageKind, arch: Arch) -> Result<&Asset, InstallError> {
        let name = artifact_name(&self.version, kind, arch);
        self.assets.iter().find(|a| a.name == name).ok_or(InstallError::MissingAsset(name))
    }
}

pub fn client() -> Result<reqwest::blocking::Client, InstallError> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(concat!("waydlp-installer/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(60 * 30))
        .build()?)
}

/// Fetch release metadata. `version = None` means the latest stable release.
pub fn fetch(version: Option<&str>) -> Result<Release, InstallError> {
    let c = client()?;
    let url = match version {
        Some(v) => format!("https://api.github.com/repos/{REPO}/releases/tags/v{}", v.trim_start_matches('v')),
        None => format!("https://api.github.com/repos/{REPO}/releases/latest"),
    };
    let r = c.get(&url).header("Accept", "application/vnd.github+json").send()?;
    if r.status().as_u16() == 404 {
        return Err(InstallError::NoRelease(version.map(|v| format!(" for version {v}")).unwrap_or_default()));
    }
    if !r.status().is_success() {
        return Err(InstallError::Http(format!("GitHub responded with HTTP {}", r.status())));
    }
    let rel: GhRelease = r.json()?;
    if version.is_none() && (rel.draft || rel.prerelease) {
        return Err(InstallError::NoRelease(String::new()));
    }
    let sums = rel
        .assets
        .iter()
        .find(|a| a.name == CHECKSUM_FILE)
        .ok_or_else(|| InstallError::MissingChecksum(CHECKSUM_FILE.into()))?;
    let text = c.get(&sums.browser_download_url).send()?.error_for_status()?.text()?;
    Ok(Release {
        version: rel.tag_name.trim_start_matches('v').to_string(),
        checksums: parse_checksums(&text).into_iter().collect(),
        assets: rel.assets,
    })
}

/// Download an asset into `dir` and verify its checksum. On mismatch the
/// file is deleted and an error returned.
pub fn download_verified(
    release: &Release,
    asset: &Asset,
    dir: &Path,
    progress: &dyn Fn(u64, Option<u64>),
) -> Result<PathBuf, InstallError> {
    let expected =
        release.checksums.get(&asset.name).ok_or_else(|| InstallError::MissingChecksum(asset.name.clone()))?;
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(&asset.name);
    let tmp = dir.join(format!("{}.part", asset.name));
    let mut r = client()?.get(&asset.browser_download_url).send()?.error_for_status()?;
    let total = r.content_length().or(Some(asset.size).filter(|s| *s > 0));
    let mut f = std::fs::File::create(&tmp)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| InstallError::Http(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        f.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    f.sync_all()?;
    drop(f);
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        let _ = std::fs::remove_file(&tmp);
        return Err(InstallError::ChecksumMismatch { file: asset.name.clone(), expected: expected.clone(), actual });
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

/// Verify a local file against an expected SHA-256.
pub fn sha256_file(p: &Path) -> Result<String, InstallError> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}
