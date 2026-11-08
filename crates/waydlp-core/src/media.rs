//! Parsing of yt-dlp `--dump-single-json` output and format analysis.
//!
//! Only fields that yt-dlp actually reported are exposed; nothing is
//! invented. Optional values stay `None` when the extractor didn't provide
//! them, and the UI is expected to hide what is unknown.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Thumbnail {
    pub url: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SubtitleTrack {
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One format as reported by yt-dlp.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FormatInfo {
    pub format_id: String,
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default)]
    pub vcodec: Option<String>,
    #[serde(default)]
    pub acodec: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub width: Option<u32>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub tbr: Option<f64>,
    #[serde(default)]
    pub abr: Option<f64>,
    #[serde(default)]
    pub vbr: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub filesize: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub filesize_approx: Option<u64>,
    #[serde(default)]
    pub format_note: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub audio_channels: Option<u32>,
    #[serde(default)]
    pub dynamic_range: Option<String>,
}

impl FormatInfo {
    fn codec_present(c: &Option<String>) -> bool {
        matches!(c.as_deref(), Some(v) if v != "none")
    }

    /// True when this is a storyboard / image-only "format".
    pub fn is_storyboard(&self) -> bool {
        self.ext.as_deref() == Some("mhtml")
            || self.protocol.as_deref() == Some("mhtml")
            || self.format_note.as_deref() == Some("storyboard")
    }

    pub fn has_video(&self) -> bool {
        if self.is_storyboard() {
            return false;
        }
        if Self::codec_present(&self.vcodec) {
            return true;
        }
        // Some extractors omit codecs but give a height.
        self.vcodec.is_none() && self.height.is_some_and(|h| h > 0)
    }

    pub fn has_audio(&self) -> bool {
        if self.is_storyboard() {
            return false;
        }
        if Self::codec_present(&self.acodec) {
            return true;
        }
        // Unknown codecs on a non-video format: assume audio (e.g. generic mp3).
        self.acodec.is_none() && self.vcodec.is_none() && self.height.is_none()
    }

    pub fn is_audio_only(&self) -> bool {
        self.has_audio() && !self.has_video()
    }

    pub fn is_video_only(&self) -> bool {
        self.has_video() && !self.has_audio()
    }

    /// Exact size if known, otherwise yt-dlp's estimate.
    pub fn size(&self) -> Option<u64> {
        self.filesize.or(self.filesize_approx)
    }

    /// Short codec family name: h264, h265, vp9, av1, aac, opus, mp3...
    pub fn video_codec_family(&self) -> Option<&'static str> {
        codec_family(self.vcodec.as_deref()?)
    }

    pub fn audio_codec_family(&self) -> Option<&'static str> {
        codec_family(self.acodec.as_deref()?)
    }

    /// A compact human-readable label, e.g. `1080p60 · mp4 · h264 · 245.7 MB`.
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.has_video() {
            match (self.height, self.fps) {
                (Some(h), Some(fps)) if fps > 30.5 => parts.push(format!("{h}p{}", fps.round() as u32)),
                (Some(h), _) => parts.push(format!("{h}p")),
                _ => parts.push("video".into()),
            }
        } else if self.has_audio() {
            match self.abr.or(self.tbr) {
                Some(a) if a > 0.0 => parts.push(format!("audio {}k", a.round() as u32)),
                _ => parts.push("audio".into()),
            }
        }
        if let Some(ext) = &self.ext {
            parts.push(ext.clone());
        }
        if let Some(v) = self.video_codec_family() {
            parts.push(v.to_string());
        }
        if let Some(a) = self.audio_codec_family() {
            parts.push(a.to_string());
        }
        if self.is_video_only() {
            parts.push("no audio".into());
        }
        if let Some(dr) = &self.dynamic_range
            && dr != "SDR"
        {
            parts.push(dr.clone());
        }
        if let Some(lang) = &self.language {
            parts.push(lang.clone());
        }
        if let Some(size) = self.size() {
            let approx = if self.filesize.is_none() { "~" } else { "" };
            parts.push(format!("{approx}{}", crate::humanize::bytes(size)));
        }
        parts.join(" · ")
    }
}

/// Map a yt-dlp codec string to a short family name.
pub fn codec_family(codec: &str) -> Option<&'static str> {
    let c = codec.to_ascii_lowercase();
    if c == "none" || c.is_empty() {
        return None;
    }
    let fam = if c.starts_with("avc") || c.starts_with("h264") {
        "h264"
    } else if c.starts_with("hev") || c.starts_with("hvc") || c.starts_with("h265") {
        "h265"
    } else if c.starts_with("vp09") || c.starts_with("vp9") {
        "vp9"
    } else if c.starts_with("vp8") {
        "vp8"
    } else if c.starts_with("av01") || c.starts_with("av1") {
        "av1"
    } else if c.starts_with("mp4a") || c.starts_with("aac") {
        "aac"
    } else if c.starts_with("opus") {
        "opus"
    } else if c.starts_with("vorbis") {
        "vorbis"
    } else if c.starts_with("mp3") {
        "mp3"
    } else if c.starts_with("flac") {
        "flac"
    } else if c.starts_with("ac-3") || c.starts_with("ac3") || c.starts_with("ec-3") {
        "ac3"
    } else {
        "other"
    };
    Some(fam)
}

/// A single media item.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MediaInfo {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub webpage_url: Option<String>,
    #[serde(default)]
    pub extractor: Option<String>,
    #[serde(default)]
    pub extractor_key: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    #[serde(default)]
    pub is_live: Option<bool>,
    #[serde(default)]
    pub live_status: Option<String>,
    #[serde(default)]
    pub upload_date: Option<String>,
    #[serde(default)]
    pub view_count: Option<u64>,
    #[serde(default)]
    pub formats: Vec<FormatInfo>,
    #[serde(default)]
    pub subtitles: BTreeMap<String, Vec<SubtitleTrack>>,
    #[serde(default)]
    pub automatic_captions: BTreeMap<String, Vec<SubtitleTrack>>,
    // Single-format extractors put format details on the root object.
    #[serde(default)]
    pub ext: Option<String>,
    #[serde(default)]
    pub format_id: Option<String>,
    #[serde(default)]
    pub vcodec: Option<String>,
    #[serde(default)]
    pub acodec: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub height: Option<u32>,
}

impl MediaInfo {
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.id)
    }

    pub fn creator(&self) -> Option<&str> {
        self.uploader.as_deref().or(self.channel.as_deref())
    }

    /// The best thumbnail URL: explicit `thumbnail`, else the largest listed.
    pub fn best_thumbnail(&self) -> Option<&str> {
        if let Some(t) = &self.thumbnail {
            return Some(t);
        }
        self.thumbnails.iter().max_by_key(|t| t.width.unwrap_or(0) * t.height.unwrap_or(0)).map(|t| t.url.as_str())
    }

    pub fn is_live_now(&self) -> bool {
        self.is_live == Some(true) || self.live_status.as_deref() == Some("is_live")
    }

    /// Formats excluding storyboards. When yt-dlp lists no formats (single-file
    /// extractors), synthesise one from the root fields that were reported.
    pub fn usable_formats(&self) -> Vec<FormatInfo> {
        let list: Vec<FormatInfo> = self.formats.iter().filter(|f| !f.is_storyboard()).cloned().collect();
        if !list.is_empty() {
            return list;
        }
        if self.ext.is_some() || self.format_id.is_some() {
            return vec![FormatInfo {
                format_id: self.format_id.clone().unwrap_or_else(|| "0".into()),
                ext: self.ext.clone(),
                vcodec: self.vcodec.clone(),
                acodec: self.acodec.clone(),
                height: self.height,
                ..Default::default()
            }];
        }
        Vec::new()
    }

    pub fn analysis(&self) -> FormatAnalysis {
        FormatAnalysis::from_formats(&self.usable_formats())
    }

    /// Languages with uploaded (non-automatic) subtitles.
    pub fn subtitle_languages(&self) -> Vec<String> {
        self.subtitles.keys().filter(|k| k.as_str() != "live_chat").cloned().collect()
    }

    pub fn auto_caption_languages(&self) -> Vec<String> {
        self.automatic_captions.keys().cloned().collect()
    }
}

/// What is actually available, derived from the real format list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatAnalysis {
    pub has_video: bool,
    pub has_audio: bool,
    /// Distinct heights of video formats, ascending.
    pub heights: Vec<u32>,
    pub max_fps: Option<f64>,
    pub video_codecs: Vec<&'static str>,
    pub audio_codecs: Vec<&'static str>,
    pub exts: Vec<String>,
    pub best_audio_kbps: Option<f64>,
    pub format_count: usize,
}

impl FormatAnalysis {
    pub fn from_formats(formats: &[FormatInfo]) -> Self {
        let mut heights = BTreeSet::new();
        let mut vcodecs = BTreeSet::new();
        let mut acodecs = BTreeSet::new();
        let mut exts = BTreeSet::new();
        let mut max_fps: Option<f64> = None;
        let mut best_abr: Option<f64> = None;
        let mut has_video = false;
        let mut has_audio = false;
        for f in formats {
            if f.has_video() {
                has_video = true;
                if let Some(h) = f.height
                    && h > 0
                {
                    heights.insert(h);
                }
                if let Some(fps) = f.fps {
                    max_fps = Some(max_fps.map_or(fps, |m: f64| m.max(fps)));
                }
                if let Some(c) = f.video_codec_family() {
                    vcodecs.insert(c);
                }
            }
            if f.has_audio() {
                has_audio = true;
                if let Some(c) = f.audio_codec_family() {
                    acodecs.insert(c);
                }
                if f.is_audio_only()
                    && let Some(a) = f.abr.or(f.tbr)
                {
                    best_abr = Some(best_abr.map_or(a, |m: f64| m.max(a)));
                }
            }
            if let Some(e) = &f.ext {
                exts.insert(e.clone());
            }
        }
        Self {
            has_video,
            has_audio,
            heights: heights.into_iter().collect(),
            max_fps,
            video_codecs: vcodecs.into_iter().collect(),
            audio_codecs: acodecs.into_iter().collect(),
            exts: exts.into_iter().collect(),
            best_audio_kbps: best_abr,
            format_count: formats.len(),
        }
    }

    /// Standard resolution steps that are actually available (the nearest
    /// available height at or below each step). Returns `(label, height)`.
    pub fn resolution_choices(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = Vec::new();
        for &h in &self.heights {
            let label = resolution_label(h);
            if !out.iter().any(|(l, _)| *l == label) {
                out.push((label, h));
            } else if let Some(last) = out.iter_mut().find(|(l, _)| *l == label) {
                last.1 = last.1.max(h);
            }
        }
        out.reverse();
        out
    }
}

/// `2160` → `2160p (4K)`.
pub fn resolution_label(height: u32) -> String {
    match height {
        4320.. => format!("{height}p (8K)"),
        2160.. => format!("{height}p (4K)"),
        1440.. => format!("{height}p (QHD)"),
        1080.. => format!("{height}p (Full HD)"),
        720.. => format!("{height}p (HD)"),
        _ => format!("{height}p"),
    }
}

/// An entry of a playlist as returned by `--flat-playlist`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlaylistEntry {
    /// 1-based position, as used by `--playlist-items`.
    #[serde(skip)]
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
    #[serde(default)]
    pub ie_key: Option<String>,
}

impl PlaylistEntry {
    pub fn display_title(&self) -> String {
        self.title.clone().or_else(|| self.id.clone()).unwrap_or_else(|| format!("Item {}", self.index))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PlaylistInfo {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub webpage_url: Option<String>,
    #[serde(default)]
    pub extractor_key: Option<String>,
    #[serde(default)]
    pub playlist_count: Option<u64>,
    /// Filled manually by [`parse_probe_json`] so `null` entries can be skipped.
    #[serde(default, skip_deserializing)]
    pub entries: Vec<PlaylistEntry>,
}

impl PlaylistInfo {
    pub fn display_title(&self) -> String {
        self.title.clone().or_else(|| self.id.clone()).unwrap_or_else(|| "Playlist".into())
    }
}

/// Result of probing a URL.
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeResult {
    Single(Box<MediaInfo>),
    Playlist(PlaylistInfo),
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("yt-dlp returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yt-dlp returned no media information")]
    Empty,
}

/// Parse the output of `yt-dlp -J [--flat-playlist]`.
pub fn parse_probe_json(json: &str) -> Result<ProbeResult, ParseError> {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Err(ParseError::Empty);
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)?;
    let kind = value.get("_type").and_then(|v| v.as_str()).unwrap_or("video");
    match kind {
        "playlist" | "multi_video" => {
            let mut pl: PlaylistInfo = serde_json::from_value(value.clone())?;
            // Entries may contain nulls for unavailable items; drop them but
            // keep the original 1-based numbering.
            let raw_entries = value.get("entries").and_then(|e| e.as_array()).cloned().unwrap_or_default();
            pl.entries = raw_entries
                .into_iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    if e.is_null() {
                        return None;
                    }
                    let mut entry: PlaylistEntry = serde_json::from_value(e).ok()?;
                    entry.index = i + 1;
                    Some(entry)
                })
                .collect();
            Ok(ProbeResult::Playlist(pl))
        }
        _ => {
            let info: MediaInfo = serde_json::from_value(value)?;
            Ok(ProbeResult::Single(Box::new(info)))
        }
    }
}

fn de_opt_u32<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(v.and_then(|v| v.as_f64()).filter(|f| *f >= 0.0).map(|f| f as u32))
}

fn de_opt_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(v.and_then(|v| v.as_f64()).filter(|f| *f >= 0.0).map(|f| f as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIDEO: &str = include_str!("../tests/fixtures/youtube_video.json");
    const PLAYLIST: &str = include_str!("../tests/fixtures/youtube_playlist_flat.json");

    fn video() -> MediaInfo {
        match parse_probe_json(VIDEO).unwrap() {
            ProbeResult::Single(m) => *m,
            _ => panic!("expected single"),
        }
    }

    #[test]
    fn parses_real_video_metadata() {
        let m = video();
        assert_eq!(m.id, "aqz-KE-bpKQ");
        assert!(m.display_title().contains("Big Buck Bunny"));
        assert_eq!(m.creator(), Some("Blender"));
        assert_eq!(m.duration, Some(635.0));
        assert_eq!(m.extractor_key.as_deref(), Some("Youtube"));
        assert!(m.best_thumbnail().is_some());
        assert!(!m.is_live_now());
    }

    #[test]
    fn excludes_storyboards() {
        let m = video();
        assert!(m.formats.iter().any(|f| f.is_storyboard()));
        assert!(m.usable_formats().iter().all(|f| !f.is_storyboard()));
    }

    #[test]
    fn analysis_reports_only_real_heights() {
        let a = video().analysis();
        assert!(a.has_video && a.has_audio);
        assert_eq!(a.heights, vec![144, 240, 360, 480, 720, 1080, 1440, 2160]);
        assert!(!a.heights.contains(&4320));
        assert_eq!(a.max_fps, Some(60.0));
        assert!(a.video_codecs.contains(&"h264"));
        assert!(a.video_codecs.contains(&"vp9"));
        assert!(a.video_codecs.contains(&"av1"));
        assert!(a.audio_codecs.contains(&"opus"));
        assert!(a.best_audio_kbps.unwrap() > 380.0);
        let choices = a.resolution_choices();
        assert_eq!(choices.first().unwrap().1, 2160);
        assert_eq!(choices.last().unwrap().1, 144);
    }

    #[test]
    fn format_flags_and_labels() {
        let m = video();
        let f18 = m.formats.iter().find(|f| f.format_id == "18").unwrap();
        assert!(f18.has_video() && f18.has_audio());
        let f140 = m.formats.iter().find(|f| f.format_id == "140").unwrap();
        assert!(f140.is_audio_only());
        let f299 = m.formats.iter().find(|f| f.format_id == "299").unwrap();
        assert!(f299.is_video_only());
        let label = f299.label();
        assert!(label.starts_with("1080p60 · mp4 · h264"), "{label}");
        assert!(label.contains("no audio"));
    }

    #[test]
    fn subtitles() {
        let m = video();
        assert_eq!(m.subtitle_languages(), vec!["de", "en"]);
        assert_eq!(m.auto_caption_languages(), vec!["fr"]);
    }

    #[test]
    fn parses_flat_playlist() {
        match parse_probe_json(PLAYLIST).unwrap() {
            ProbeResult::Playlist(p) => {
                assert_eq!(p.entries.len(), 4);
                assert_eq!(p.entries[0].index, 1);
                assert_eq!(p.entries[3].index, 4);
                assert!(p.entries[0].url.as_deref().unwrap().starts_with("https://"));
                assert!(p.display_title().contains("Blender"));
            }
            _ => panic!("expected playlist"),
        }
    }

    #[test]
    fn playlist_null_entries_keep_numbering() {
        let json = r#"{"_type":"playlist","title":"x","entries":[{"id":"a"},null,{"id":"c"}]}"#;
        match parse_probe_json(json).unwrap() {
            ProbeResult::Playlist(p) => {
                assert_eq!(p.entries.len(), 2);
                assert_eq!(p.entries[1].index, 3);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn single_file_extractor_synthesises_format() {
        let json = r#"{"id":"x","title":"Direct","ext":"mp3","vcodec":"none","acodec":"mp3"}"#;
        let ProbeResult::Single(m) = parse_probe_json(json).unwrap() else { panic!() };
        let f = m.usable_formats();
        assert_eq!(f.len(), 1);
        assert!(f[0].is_audio_only());
    }

    #[test]
    fn empty_and_invalid() {
        assert!(matches!(parse_probe_json(""), Err(ParseError::Empty)));
        assert!(matches!(parse_probe_json("null"), Err(ParseError::Empty)));
        assert!(matches!(parse_probe_json("{not json"), Err(ParseError::Json(_))));
    }

    #[test]
    fn codec_families() {
        assert_eq!(codec_family("avc1.64002A"), Some("h264"));
        assert_eq!(codec_family("hvc1.1.6.L93"), Some("h265"));
        assert_eq!(codec_family("vp09.00.40.08"), Some("vp9"));
        assert_eq!(codec_family("av01.0.08M.08"), Some("av1"));
        assert_eq!(codec_family("mp4a.40.2"), Some("aac"));
        assert_eq!(codec_family("none"), None);
    }
}
