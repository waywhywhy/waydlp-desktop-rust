//! Discovery of yt-dlp, FFmpeg and FFprobe.
//!
//! Search order for each tool:
//! 1. A path the user configured explicitly.
//! 2. A copy managed by waydlp (installed via a verified download).
//! 3. A copy bundled next to the waydlp executable (portable builds).
//! 4. The system `PATH` (distribution packages, pip, winget, ...).

use crate::process::run_capture;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSource {
    Custom,
    Managed,
    Bundled,
    System,
}

impl ToolSource {
    pub fn label(self) -> &'static str {
        match self {
            ToolSource::Custom => "custom path",
            ToolSource::Managed => "managed by waydlp",
            ToolSource::Bundled => "bundled",
            ToolSource::System => "system",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub path: PathBuf,
    pub version: Option<String>,
    pub source: ToolSource,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolStatus {
    pub yt_dlp: Option<ToolInfo>,
    pub ffmpeg: Option<ToolInfo>,
    pub ffprobe: Option<ToolInfo>,
    /// A configured custom path that doesn't work, with the reason.
    pub problems: Vec<String>,
}

impl ToolStatus {
    /// Value for yt-dlp's `--ffmpeg-location`: the directory when ffprobe sits
    /// next to ffmpeg, otherwise the ffmpeg binary itself.
    pub fn ffmpeg_location(&self) -> Option<PathBuf> {
        let ff = self.ffmpeg.as_ref()?;
        if ff.source == ToolSource::System {
            return None; // yt-dlp finds it on PATH itself
        }
        let dir = ff.path.parent()?;
        if self.ffprobe.as_ref().is_some_and(|p| p.path.parent() == Some(dir)) {
            Some(dir.to_path_buf())
        } else {
            Some(ff.path.clone())
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolSearch {
    pub custom_yt_dlp: Option<PathBuf>,
    /// Either the ffmpeg binary or the directory that contains it.
    pub custom_ffmpeg: Option<PathBuf>,
    pub managed_dir: Option<PathBuf>,
    pub bundled_dir: Option<PathBuf>,
}

pub fn exe_name(base: &str) -> String {
    if cfg!(windows) { format!("{base}.exe") } else { base.to_string() }
}

fn existing_file(p: &Path) -> Option<PathBuf> {
    if p.is_file() { Some(p.to_path_buf()) } else { None }
}

fn find_tool(
    name: &str,
    custom: Option<&Path>,
    search: &ToolSearch,
    problems: &mut Vec<String>,
) -> Option<(PathBuf, ToolSource)> {
    let exe = exe_name(name);
    if let Some(c) = custom {
        let candidate = if c.is_dir() { c.join(&exe) } else { c.to_path_buf() };
        match existing_file(&candidate) {
            Some(p) => return Some((p, ToolSource::Custom)),
            None => problems.push(format!(
                "The configured {name} path \"{}\" does not exist; falling back to automatic detection.",
                candidate.display()
            )),
        }
    }
    if let Some(dir) = &search.managed_dir
        && let Some(p) = existing_file(&dir.join(&exe))
    {
        return Some((p, ToolSource::Managed));
    }
    if let Some(dir) = &search.bundled_dir {
        for sub in [dir.clone(), dir.join("bin"), dir.join("tools")] {
            if let Some(p) = existing_file(&sub.join(&exe)) {
                return Some((p, ToolSource::Bundled));
            }
        }
    }
    which::which(name).ok().map(|p| (p, ToolSource::System))
}

async fn version_of(path: &Path, flag: &str) -> Option<String> {
    let out = run_capture(path, [flag], Duration::from_secs(60)).await.ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim().to_string();
    if first.is_empty() { None } else { Some(first) }
}

/// Extract `7.1` from `ffmpeg version 7.1-full_build-www.gyan.dev Copyright...`.
pub fn parse_ffmpeg_version(line: &str) -> String {
    let mut it = line.split_whitespace();
    while let Some(tok) = it.next() {
        if tok == "version" {
            return it.next().unwrap_or(line).to_string();
        }
    }
    line.to_string()
}

pub async fn detect_tools(search: &ToolSearch) -> ToolStatus {
    let mut problems = Vec::new();
    let yt = find_tool("yt-dlp", search.custom_yt_dlp.as_deref(), search, &mut problems);
    let ff = find_tool("ffmpeg", search.custom_ffmpeg.as_deref(), search, &mut problems);
    // ffprobe: prefer the directory ffmpeg was found in.
    let fp = ff
        .as_ref()
        .and_then(|(p, src)| p.parent().and_then(|d| existing_file(&d.join(exe_name("ffprobe")))).map(|p| (p, *src)))
        .or_else(|| find_tool("ffprobe", None, search, &mut Vec::new()));

    let yt_dlp = match yt {
        Some((path, source)) => {
            let version = version_of(&path, "--version").await;
            if version.is_none() {
                problems
                    .push(format!("{} did not report a version (it may be slow to start or broken).", path.display()));
            }
            Some(ToolInfo { path, version, source })
        }
        None => None,
    };
    let ffmpeg = match ff {
        Some((path, source)) => {
            let version = version_of(&path, "-version").await.map(|l| parse_ffmpeg_version(&l));
            Some(ToolInfo { path, version, source })
        }
        None => None,
    };
    let ffprobe = match fp {
        Some((path, source)) => {
            let version = version_of(&path, "-version").await.map(|l| parse_ffmpeg_version(&l));
            Some(ToolInfo { path, version, source })
        }
        None => None,
    };
    ToolStatus { yt_dlp, ffmpeg, ffprobe, problems }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_version_parsing() {
        assert_eq!(
            parse_ffmpeg_version("ffmpeg version 7.1-full_build-www.gyan.dev Copyright (c) 2000-2024"),
            "7.1-full_build-www.gyan.dev"
        );
        assert_eq!(parse_ffmpeg_version("ffprobe version n8.0 Copyright"), "n8.0");
        assert_eq!(parse_ffmpeg_version("weird"), "weird");
    }

    #[test]
    fn ffmpeg_location_prefers_directory() {
        let dir = PathBuf::from("/opt/w/bin");
        let s = ToolStatus {
            yt_dlp: None,
            ffmpeg: Some(ToolInfo { path: dir.join("ffmpeg"), version: None, source: ToolSource::Managed }),
            ffprobe: Some(ToolInfo { path: dir.join("ffprobe"), version: None, source: ToolSource::Managed }),
            problems: vec![],
        };
        assert_eq!(s.ffmpeg_location(), Some(dir.clone()));
        let s2 = ToolStatus { ffprobe: None, ..s.clone() };
        assert_eq!(s2.ffmpeg_location(), Some(dir.join("ffmpeg")));
        let mut s3 = s;
        s3.ffmpeg.as_mut().unwrap().source = ToolSource::System;
        assert_eq!(s3.ffmpeg_location(), None);
    }

    #[tokio::test]
    async fn custom_path_problem_is_reported() {
        let search =
            ToolSearch { custom_yt_dlp: Some(PathBuf::from("/definitely/not/here/yt-dlp")), ..Default::default() };
        let status = detect_tools(&search).await;
        assert!(status.problems.iter().any(|p| p.contains("does not exist")));
    }
}
