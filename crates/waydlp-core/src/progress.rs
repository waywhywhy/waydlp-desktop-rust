//! Machine-readable progress from yt-dlp.
//!
//! We ask yt-dlp to print progress with our own templates so that parsing
//! does not depend on its human-oriented output, which changes over time.
//! Every number shown in the UI comes from these lines; nothing is simulated.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DL_PREFIX: &str = "[waydlp:dl]";
pub const PP_PREFIX: &str = "[waydlp:pp]";
pub const ITEM_PREFIX: &str = "[waydlp:item]";
pub const FILE_PREFIX: &str = "[waydlp:file]";

pub const DOWNLOAD_TEMPLATE: &str = "download:[waydlp:dl]%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(progress.fragment_index)s|%(progress.fragment_count)s|%(info.vcodec)s|%(info.acodec)s";
pub const POSTPROCESS_TEMPLATE: &str = "postprocess:[waydlp:pp]%(progress.status)s|%(progress.postprocessor)s";
pub const ITEM_PRINT: &str = "video:[waydlp:item]%(playlist_index|NA)s|%(n_entries|NA)s|%(id|NA)s|%(title|NA)s";
pub const FILE_PRINT: &str = "after_move:[waydlp:file]%(filepath)s";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamKind {
    Video,
    Audio,
    Combined,
    Unknown,
}

impl StreamKind {
    fn from_codecs(v: Option<&str>, a: Option<&str>) -> Self {
        let has = |c: Option<&str>| matches!(c, Some(c) if c != "none" && c != "NA");
        match (has(v), has(a)) {
            (true, true) => StreamKind::Combined,
            (true, false) => StreamKind::Video,
            (false, true) => StreamKind::Audio,
            _ => StreamKind::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StreamKind::Video => "Downloading video stream",
            StreamKind::Audio => "Downloading audio stream",
            StreamKind::Combined | StreamKind::Unknown => "Downloading",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// `downloading`, `finished`, `error`.
    pub status: String,
    pub downloaded: Option<u64>,
    pub total: Option<u64>,
    /// True if `total` is yt-dlp's estimate rather than an exact size.
    pub total_is_estimate: bool,
    pub speed: Option<f64>,
    pub eta: Option<u64>,
    pub fragment: Option<(u64, u64)>,
    pub stream: StreamKind,
}

impl DownloadProgress {
    /// Fraction 0..=1 when the total is known.
    pub fn fraction(&self) -> Option<f32> {
        if self.status == "finished" {
            return Some(1.0);
        }
        match (self.downloaded, self.total) {
            (Some(d), Some(t)) if t > 0 => Some((d as f64 / t as f64).clamp(0.0, 1.0) as f32),
            _ => match self.fragment {
                Some((i, n)) if n > 0 => Some((i as f64 / n as f64).clamp(0.0, 1.0) as f32),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgressEvent {
    Download(DownloadProgress),
    PostProcess { status: String, processor: String },
    Item { index: Option<u32>, count: Option<u32>, id: Option<String>, title: Option<String> },
    FinalFile(PathBuf),
    Warning(String),
    Error(String),
    Log(String),
}

fn na(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() || s == "NA" || s == "None" { None } else { Some(s) }
}

fn num_u64(s: &str) -> Option<u64> {
    na(s)?.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| v as u64)
}

fn num_f64(s: &str) -> Option<f64> {
    na(s)?.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

/// Parse one line of yt-dlp stdout/stderr.
pub fn parse_line(line: &str) -> ProgressEvent {
    let line = line.trim_end_matches(['\r', '\n']);
    if let Some(rest) = line.strip_prefix(DL_PREFIX) {
        let p: Vec<&str> = rest.split('|').collect();
        let get = |i: usize| p.get(i).copied().unwrap_or("NA");
        let exact = num_u64(get(2));
        let estimate = num_u64(get(3));
        let frag = match (num_u64(get(6)), num_u64(get(7))) {
            (Some(i), Some(n)) => Some((i, n)),
            _ => None,
        };
        return ProgressEvent::Download(DownloadProgress {
            status: na(get(0)).unwrap_or("downloading").to_string(),
            downloaded: num_u64(get(1)),
            total: exact.or(estimate),
            total_is_estimate: exact.is_none() && estimate.is_some(),
            speed: num_f64(get(4)),
            eta: num_u64(get(5)),
            fragment: frag,
            stream: StreamKind::from_codecs(na(get(8)), na(get(9))),
        });
    }
    if let Some(rest) = line.strip_prefix(PP_PREFIX) {
        let mut it = rest.splitn(2, '|');
        let status = it.next().unwrap_or("").to_string();
        let processor = it.next().unwrap_or("").to_string();
        return ProgressEvent::PostProcess { status, processor };
    }
    if let Some(rest) = line.strip_prefix(ITEM_PREFIX) {
        let mut it = rest.splitn(4, '|');
        let index = it.next().and_then(na).and_then(|s| s.parse().ok());
        let count = it.next().and_then(na).and_then(|s| s.parse().ok());
        let id = it.next().and_then(na).map(str::to_string);
        let title = it.next().and_then(na).map(str::to_string);
        return ProgressEvent::Item { index, count, id, title };
    }
    if let Some(rest) = line.strip_prefix(FILE_PREFIX) {
        return ProgressEvent::FinalFile(PathBuf::from(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("WARNING:") {
        return ProgressEvent::Warning(rest.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("ERROR:") {
        return ProgressEvent::Error(rest.trim().to_string());
    }
    ProgressEvent::Log(line.to_string())
}

/// Friendly name for a yt-dlp post-processor.
pub fn postprocessor_label(name: &str) -> String {
    let n = name.trim();
    let label = match n {
        "Merger" | "FFmpegMerger" => "Merging video and audio",
        "ExtractAudio" | "FFmpegExtractAudio" => "Converting audio",
        "VideoRemuxer" | "FFmpegVideoRemuxer" => "Remuxing (no quality loss)",
        "VideoConvertor" | "FFmpegVideoConvertor" => "Re-encoding video",
        "EmbedThumbnail" => "Embedding thumbnail",
        "Metadata" | "FFmpegMetadata" => "Writing metadata",
        "EmbedSubtitle" | "FFmpegEmbedSubtitle" => "Embedding subtitles",
        "SubtitlesConvertor" | "FFmpegSubtitlesConvertor" => "Converting subtitles",
        "FixupM3u8"
        | "FFmpegFixupM3u8"
        | "FixupM4a"
        | "FFmpegFixupM4a"
        | "FixupStretched"
        | "FFmpegFixupStretched"
        | "FixupDuplicateMoov"
        | "FixupTimestamp" => "Fixing up file",
        "MoveFiles" | "MoveFilesAfterDownload" => "Moving into place",
        "ModifyChapters" | "SponsorBlock" => "Processing chapters",
        "ThumbnailsConvertor" | "FFmpegThumbnailsConvertor" => "Converting thumbnail",
        _ => return format!("Processing ({n})"),
    };
    label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_download_line() {
        let e = parse_line("[waydlp:dl]downloading|1048576|4194304|NA|524288.5|6|NA|NA|avc1.64002A|none");
        let ProgressEvent::Download(p) = e else { panic!() };
        assert_eq!(p.downloaded, Some(1_048_576));
        assert_eq!(p.total, Some(4_194_304));
        assert!(!p.total_is_estimate);
        assert_eq!(p.speed, Some(524_288.5));
        assert_eq!(p.eta, Some(6));
        assert_eq!(p.stream, StreamKind::Video);
        assert_eq!(p.fraction(), Some(0.25));
    }

    #[test]
    fn uses_estimate_and_fragments() {
        let e = parse_line("[waydlp:dl]downloading|500|NA|1000.0|NA|NA|3|12|none|opus");
        let ProgressEvent::Download(p) = e else { panic!() };
        assert_eq!(p.total, Some(1000));
        assert!(p.total_is_estimate);
        assert_eq!(p.speed, None);
        assert_eq!(p.fragment, Some((3, 12)));
        assert_eq!(p.stream, StreamKind::Audio);
        assert_eq!(p.fraction(), Some(0.5));
    }

    #[test]
    fn unknown_total_has_no_fraction() {
        let ProgressEvent::Download(p) = parse_line("[waydlp:dl]downloading|500|NA|NA|NA|NA|NA|NA|NA|NA") else {
            panic!()
        };
        assert_eq!(p.fraction(), None);
        let ProgressEvent::Download(p) = parse_line("[waydlp:dl]finished|500|NA|NA|NA|NA|NA|NA|NA|NA") else {
            panic!()
        };
        assert_eq!(p.fraction(), Some(1.0));
    }

    #[test]
    fn parses_postprocess_item_and_file() {
        assert_eq!(
            parse_line("[waydlp:pp]started|Merger"),
            ProgressEvent::PostProcess { status: "started".into(), processor: "Merger".into() }
        );
        assert_eq!(
            parse_line("[waydlp:item]2|10|abc|A | title | with pipes"),
            ProgressEvent::Item {
                index: Some(2),
                count: Some(10),
                id: Some("abc".into()),
                title: Some("A | title | with pipes".into())
            }
        );
        assert_eq!(
            parse_line("[waydlp:item]NA|NA|abc|Single"),
            ProgressEvent::Item { index: None, count: None, id: Some("abc".into()), title: Some("Single".into()) }
        );
        assert_eq!(
            parse_line("[waydlp:file]C:\\Users\\me\\Videos\\x [id].mp4\r"),
            ProgressEvent::FinalFile(PathBuf::from("C:\\Users\\me\\Videos\\x [id].mp4"))
        );
    }

    #[test]
    fn parses_warnings_errors_and_logs() {
        assert_eq!(parse_line("WARNING: something"), ProgressEvent::Warning("something".into()));
        assert_eq!(
            parse_line("ERROR: [youtube] x: Private video"),
            ProgressEvent::Error("[youtube] x: Private video".into())
        );
        assert_eq!(parse_line("hello"), ProgressEvent::Log("hello".into()));
    }

    #[test]
    fn pp_labels() {
        assert_eq!(postprocessor_label("Merger"), "Merging video and audio");
        assert_eq!(postprocessor_label("Unknown"), "Processing (Unknown)");
    }
}
