//! Checking GitHub Releases for a newer waydlp version.
//!
//! This only *checks*. How to update is decided by the install kind (see
//! `waydlp-platform`): package-managed installs are never modified by waydlp.

use serde::Deserialize;

pub const DESKTOP_REPO: &str = "waywhywhy/waydlp-desktop-rust";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub url: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub is_newer: bool,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("No releases have been published yet.")]
    NoRelease,
    #[error("GitHub responded with HTTP {0}.")]
    Status(u16),
}

/// Is `latest` a newer semantic version than `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    let l = semver_like(latest);
    let c = semver_like(current);
    match (l, c) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

fn semver_like(v: &str) -> Option<(u64, u64, u64, bool)> {
    let v = v.trim().trim_start_matches('v');
    let (core, pre) = match v.split_once('-') {
        Some((c, _)) => (c, true),
        None => (v, false),
    };
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = it.next()??;
    let minor = it.next().flatten().unwrap_or(0);
    let patch = it.next().flatten().unwrap_or(0);
    // A pre-release sorts before the corresponding release.
    Some((major, minor, patch, !pre))
}

pub async fn check_latest(client: &reqwest::Client, current_version: &str) -> Result<ReleaseInfo, UpdateError> {
    let url = format!("https://api.github.com/repos/{DESKTOP_REPO}/releases/latest");
    let r = client.get(&url).header("Accept", "application/vnd.github+json").send().await?;
    if r.status().as_u16() == 404 {
        return Err(UpdateError::NoRelease);
    }
    if !r.status().is_success() {
        return Err(UpdateError::Status(r.status().as_u16()));
    }
    let rel: GhRelease = r.json().await?;
    if rel.draft || rel.prerelease {
        return Err(UpdateError::NoRelease);
    }
    let version = rel.tag_name.trim_start_matches('v').to_string();
    Ok(ReleaseInfo {
        is_newer: is_newer(&version, current_version),
        version,
        url: rel.html_url,
        notes: rel.body.unwrap_or_default(),
        published_at: rel.published_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v1.2.0", "1.1.9"));
        assert!(is_newer("1.0.0", "1.0.0-rc.1"));
        assert!(!is_newer("1.0.0-rc.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
        assert!(is_newer("2", "1.9.9"));
    }
}
