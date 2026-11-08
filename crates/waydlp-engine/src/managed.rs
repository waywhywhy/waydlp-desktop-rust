//! Verified installation of waydlp-managed copies of yt-dlp and FFmpeg.
//!
//! Integrity model: every file is downloaded over HTTPS from the official
//! GitHub releases of the respective project and its SHA-256 digest is
//! compared with the checksum list published in the same release before the
//! file is moved into place. Anything that fails verification is deleted.
//!
//! Limitation (documented): the detached GPG signature of yt-dlp's checksum
//! file is not verified yet; trust is anchored in GitHub's TLS certificate.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use waydlp_core::artifacts::parse_checksums;

pub const YT_DLP_RELEASES: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
pub const YT_DLP_API_LATEST: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
pub const FFMPEG_RELEASES: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest";

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("No prebuilt binary is available for this platform ({0}).")]
    UnsupportedPlatform(String),
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("The checksum list does not contain {0}.")]
    MissingChecksum(String),
    #[error("Checksum mismatch for {file}: expected {expected}, got {actual}. The file was discarded.")]
    ChecksumMismatch { file: String, expected: String, actual: String },
    #[error("The archive is invalid: {0}")]
    Archive(String),
    #[error("Server responded with HTTP {0} for {1}")]
    Status(u16, String),
}

/// Asset name of the standalone yt-dlp build for this platform.
pub fn yt_dlp_asset() -> Result<&'static str, InstallError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("yt-dlp.exe"),
        ("windows", "aarch64") => Ok("yt-dlp_arm64.exe"),
        ("windows", "x86") => Ok("yt-dlp_x86.exe"),
        ("linux", "x86_64") => Ok("yt-dlp_linux"),
        ("linux", "aarch64") => Ok("yt-dlp_linux_aarch64"),
        (os, arch) => Err(InstallError::UnsupportedPlatform(format!("{os}/{arch}"))),
    }
}

/// Asset name of the FFmpeg build (Windows only; Linux users should use
/// their distribution's package, which integrates with system updates).
pub fn ffmpeg_asset() -> Result<&'static str, InstallError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("ffmpeg-master-latest-win64-gpl.zip"),
        ("windows", "aarch64") => Ok("ffmpeg-master-latest-winarm64-gpl.zip"),
        (os, arch) => Err(InstallError::UnsupportedPlatform(format!("{os}/{arch}"))),
    }
}

pub fn ffmpeg_download_supported() -> bool {
    ffmpeg_asset().is_ok()
}

/// Progress callback: `(downloaded_bytes, total_bytes_if_known)`.
pub type ProgressFn<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

pub struct ManagedTools {
    client: reqwest::Client,
    bin_dir: PathBuf,
}

impl ManagedTools {
    pub fn new(client: reqwest::Client, bin_dir: PathBuf) -> Self {
        Self { client, bin_dir }
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    async fn get_text(&self, url: &str) -> Result<String, InstallError> {
        let r = self.client.get(url).send().await?;
        if !r.status().is_success() {
            return Err(InstallError::Status(r.status().as_u16(), url.to_string()));
        }
        Ok(r.text().await?)
    }

    async fn expected_hash(&self, sums_url: &str, file: &str) -> Result<String, InstallError> {
        let text = self.get_text(sums_url).await?;
        parse_checksums(&text)
            .into_iter()
            .find(|(n, _)| n == file)
            .map(|(_, h)| h)
            .ok_or_else(|| InstallError::MissingChecksum(file.to_string()))
    }

    /// Stream `url` into `dest`, hashing while writing. Returns the hex digest.
    async fn download_to(&self, url: &str, dest: &Path, progress: ProgressFn<'_>) -> Result<String, InstallError> {
        let r = self.client.get(url).send().await?;
        if !r.status().is_success() {
            return Err(InstallError::Status(r.status().as_u16(), url.to_string()));
        }
        let total = r.content_length();
        let mut file = std::fs::File::create(dest)?;
        let mut hasher = Sha256::new();
        let mut done: u64 = 0;
        let mut stream = r.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            file.write_all(&chunk)?;
            done += chunk.len() as u64;
            progress(done, total);
        }
        file.flush()?;
        file.sync_all()?;
        Ok(hex::encode(hasher.finalize()))
    }

    fn verify(file: &str, expected: &str, actual: &str, tmp: &Path) -> Result<(), InstallError> {
        if expected.eq_ignore_ascii_case(actual) {
            Ok(())
        } else {
            let _ = std::fs::remove_file(tmp);
            Err(InstallError::ChecksumMismatch {
                file: file.to_string(),
                expected: expected.to_string(),
                actual: actual.to_string(),
            })
        }
    }

    /// Download and verify the latest yt-dlp into the managed directory.
    pub async fn install_yt_dlp(&self, progress: ProgressFn<'_>) -> Result<PathBuf, InstallError> {
        let asset = yt_dlp_asset()?;
        std::fs::create_dir_all(&self.bin_dir)?;
        let expected = self.expected_hash(&format!("{YT_DLP_RELEASES}/SHA2-256SUMS"), asset).await?;
        let tmp = self.bin_dir.join(".yt-dlp.download");
        let actual = self.download_to(&format!("{YT_DLP_RELEASES}/{asset}"), &tmp, progress).await?;
        Self::verify(asset, &expected, &actual, &tmp)?;
        make_executable(&tmp)?;
        let dest = self.bin_dir.join(crate::tools::exe_name("yt-dlp"));
        replace_file(&tmp, &dest)?;
        tracing::info!(path = %dest.display(), "installed managed yt-dlp");
        Ok(dest)
    }

    /// Download, verify and extract FFmpeg + FFprobe (Windows).
    pub async fn install_ffmpeg(&self, progress: ProgressFn<'_>) -> Result<(PathBuf, PathBuf), InstallError> {
        let asset = ffmpeg_asset()?;
        std::fs::create_dir_all(&self.bin_dir)?;
        let expected = self.expected_hash(&format!("{FFMPEG_RELEASES}/checksums.sha256"), asset).await?;
        let tmp = self.bin_dir.join(".ffmpeg.zip.download");
        let actual = self.download_to(&format!("{FFMPEG_RELEASES}/{asset}"), &tmp, progress).await?;
        Self::verify(asset, &expected, &actual, &tmp)?;

        let bin_dir = self.bin_dir.clone();
        let tmp2 = tmp.clone();
        let result = tokio::task::spawn_blocking(move || extract_ffmpeg(&tmp2, &bin_dir))
            .await
            .map_err(|e| InstallError::Archive(e.to_string()))?;
        let _ = std::fs::remove_file(&tmp);
        let paths = result?;
        tracing::info!(dir = %self.bin_dir.display(), "installed managed FFmpeg");
        Ok(paths)
    }

    /// Latest yt-dlp release tag, e.g. `2026.10.26`.
    pub async fn latest_yt_dlp_version(&self) -> Result<String, InstallError> {
        #[derive(serde::Deserialize)]
        struct Rel {
            tag_name: String,
        }
        let r = self.client.get(YT_DLP_API_LATEST).send().await?;
        if !r.status().is_success() {
            return Err(InstallError::Status(r.status().as_u16(), YT_DLP_API_LATEST.into()));
        }
        Ok(r.json::<Rel>().await?.tag_name)
    }
}

/// Compare yt-dlp's date-based versions (`2026.07.04`, `2026.07.04.1`).
pub fn yt_dlp_version_is_newer(latest: &str, installed: &str) -> bool {
    let parse =
        |s: &str| -> Vec<u64> { s.trim().trim_start_matches('v').split('.').filter_map(|p| p.parse().ok()).collect() };
    parse(latest) > parse(installed)
}

fn extract_ffmpeg(zip_path: &Path, bin_dir: &Path) -> Result<(PathBuf, PathBuf), InstallError> {
    let f = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| InstallError::Archive(e.to_string()))?;
    let mut found = (None, None);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| InstallError::Archive(e.to_string()))?;
        let name = entry.name().map_err(|e| InstallError::Archive(e.to_string()))?.replace('\\', "/");
        let target = if name.ends_with("/bin/ffmpeg.exe") {
            "ffmpeg.exe"
        } else if name.ends_with("/bin/ffprobe.exe") {
            "ffprobe.exe"
        } else {
            continue;
        };
        // Write to a temp name first, then replace atomically.
        let tmp = bin_dir.join(format!(".{target}.tmp"));
        {
            let mut out = std::fs::File::create(&tmp)?;
            std::io::copy(&mut entry, &mut out)?;
            out.sync_all()?;
        }
        let dest = bin_dir.join(target);
        replace_file(&tmp, &dest)?;
        if target == "ffmpeg.exe" {
            found.0 = Some(dest);
        } else {
            found.1 = Some(dest);
        }
    }
    match found {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(InstallError::Archive("ffmpeg.exe/ffprobe.exe not found in archive".into())),
    }
}

fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        // On Windows rename fails if the target exists and is in use; remove first.
        let _ = std::fs::remove_file(to);
    }
    std::fs::rename(from, to)
}

#[cfg(unix)]
fn make_executable(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(p)?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(p, perm)
}

#[cfg(not(unix))]
fn make_executable(_p: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(yt_dlp_version_is_newer("2026.10.26", "2026.07.04"));
        assert!(!yt_dlp_version_is_newer("2026.07.04", "2026.07.04"));
        assert!(yt_dlp_version_is_newer("2026.07.04.1", "2026.07.04"));
        assert!(!yt_dlp_version_is_newer("2025.01.01", "2026.07.04"));
    }

    #[test]
    fn verify_rejects_and_deletes_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("x");
        std::fs::write(&tmp, b"data").unwrap();
        let r = ManagedTools::verify("x", &"0".repeat(64), &"1".repeat(64), &tmp);
        assert!(matches!(r, Err(InstallError::ChecksumMismatch { .. })));
        assert!(!tmp.exists());
        assert!(ManagedTools::verify("x", "ABC", "abc", &tmp).is_ok());
    }

    #[test]
    fn extracts_ffmpeg_binaries_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("f.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let opt = zip::write::SimpleFileOptions::default();
            for (name, data) in [
                ("ffmpeg-x/bin/ffmpeg.exe", b"FF".as_slice()),
                ("ffmpeg-x/bin/ffprobe.exe", b"FP".as_slice()),
                ("ffmpeg-x/doc/readme.txt", b"no".as_slice()),
            ] {
                z.start_file(name, opt).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        let out = dir.path().join("bin");
        std::fs::create_dir_all(&out).unwrap();
        let (a, b) = extract_ffmpeg(&zip_path, &out).unwrap();
        assert_eq!(std::fs::read(a).unwrap(), b"FF");
        assert_eq!(std::fs::read(b).unwrap(), b"FP");
        assert!(!out.join("readme.txt").exists());
    }
}
