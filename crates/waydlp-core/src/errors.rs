//! Classification of yt-dlp / FFmpeg failures into readable messages.
//!
//! The raw output is always kept in [`FailureReport::details`] so it can be
//! shown in an expandable diagnostic view.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    UnsupportedUrl,
    Unavailable,
    Private,
    AgeRestricted,
    LoginRequired,
    GeoRestricted,
    LiveNotStarted,
    Drm,
    ExtractorOutdated,
    RateLimited,
    FfmpegMissing,
    FormatUnavailable,
    DiskFull,
    PermissionDenied,
    InvalidPath,
    Network,
    Timeout,
    ConversionFailed,
    YtDlpMissing,
    Cancelled,
    Unknown,
}

impl ErrorKind {
    /// Whether retrying automatically has a realistic chance of succeeding.
    pub fn is_transient(self) -> bool {
        matches!(self, ErrorKind::Network | ErrorKind::Timeout | ErrorKind::RateLimited | ErrorKind::Unknown)
    }

    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::UnsupportedUrl => "This link isn't supported",
            ErrorKind::Unavailable => "The media is unavailable",
            ErrorKind::Private => "The media is private",
            ErrorKind::AgeRestricted => "Age-restricted content",
            ErrorKind::LoginRequired => "Sign-in required",
            ErrorKind::GeoRestricted => "Not available in your region",
            ErrorKind::LiveNotStarted => "The live stream hasn't started",
            ErrorKind::Drm => "DRM-protected content",
            ErrorKind::ExtractorOutdated => "yt-dlp may be out of date",
            ErrorKind::RateLimited => "The site is rate-limiting requests",
            ErrorKind::FfmpegMissing => "FFmpeg is required",
            ErrorKind::FormatUnavailable => "The selected format isn't available",
            ErrorKind::DiskFull => "The disk is full",
            ErrorKind::PermissionDenied => "Permission denied",
            ErrorKind::InvalidPath => "Invalid output location",
            ErrorKind::Network => "Network problem",
            ErrorKind::Timeout => "The connection timed out",
            ErrorKind::ConversionFailed => "Conversion failed",
            ErrorKind::YtDlpMissing => "yt-dlp was not found",
            ErrorKind::Cancelled => "Cancelled",
            ErrorKind::Unknown => "The download failed",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            ErrorKind::UnsupportedUrl => {
                "yt-dlp doesn't recognise this website or page. Check the link, or see Supported Websites."
            }
            ErrorKind::Unavailable => "It may have been removed or made unavailable by the uploader.",
            ErrorKind::Private => "Only people the uploader has shared it with can access it.",
            ErrorKind::AgeRestricted => {
                "The site requires a signed-in account. You can import a cookies file in Settings » yt-dlp & FFmpeg."
            }
            ErrorKind::LoginRequired => {
                "The site requires a signed-in account. You can import a cookies file in Settings » yt-dlp & FFmpeg."
            }
            ErrorKind::GeoRestricted => {
                "The uploader limited it to other countries. A proxy can be configured in Settings » yt-dlp & FFmpeg."
            }
            ErrorKind::LiveNotStarted => "Try again once the stream is live.",
            ErrorKind::Drm => "waydlp does not and will not circumvent DRM protection.",
            ErrorKind::ExtractorOutdated => {
                "The website probably changed. Update yt-dlp in Settings » yt-dlp & FFmpeg, then retry."
            }
            ErrorKind::RateLimited => "Wait a while before retrying, or lower the number of concurrent downloads.",
            ErrorKind::FfmpegMissing => {
                "Merging, conversion and embedding need FFmpeg. Install it from Settings » yt-dlp & FFmpeg."
            }
            ErrorKind::FormatUnavailable => "Analyse the link again and choose one of the listed qualities.",
            ErrorKind::DiskFull => "Free up space or choose another download folder.",
            ErrorKind::PermissionDenied => "Choose a download folder you are allowed to write to.",
            ErrorKind::InvalidPath => "Check the download folder and filename template in Settings.",
            ErrorKind::Network => "Check your internet connection and try again.",
            ErrorKind::Timeout => "The site took too long to respond. Try again later.",
            ErrorKind::ConversionFailed => {
                "FFmpeg couldn't convert the file. Try a different output format or disable re-encoding."
            }
            ErrorKind::YtDlpMissing => "Install yt-dlp from Settings » yt-dlp & FFmpeg, or set its path.",
            ErrorKind::Cancelled => "The download was cancelled.",
            ErrorKind::Unknown => "Open the technical details for more information.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureReport {
    pub kind: ErrorKind,
    /// The most relevant single error line.
    pub summary: String,
    /// Full technical output (redacted).
    pub details: String,
}

impl FailureReport {
    pub fn new(kind: ErrorKind, summary: impl Into<String>, details: impl Into<String>) -> Self {
        Self { kind, summary: summary.into(), details: redact(&details.into()) }
    }
}

const RULES: &[(ErrorKind, &[&str])] = &[
    (
        ErrorKind::Drm,
        &["drm protected", "this video is drm", "is drm protected", "the requested site is known to use drm"],
    ),
    (ErrorKind::UnsupportedUrl, &["unsupported url", "is not a valid url", "no suitable extractor"]),
    (ErrorKind::Private, &["private video", "this video is private", "is private"]),
    (
        ErrorKind::AgeRestricted,
        &["confirm your age", "age-restricted", "age restricted", "inappropriate for some users"],
    ),
    (
        ErrorKind::LoginRequired,
        &[
            "sign in to confirm",
            "login required",
            "log in to",
            "requires authentication",
            "only available for registered users",
            "use --cookies",
            "account authentication is required",
            "members-only",
            "join this channel",
        ],
    ),
    (
        ErrorKind::GeoRestricted,
        &[
            "available in your country",
            "geo restrict",
            "geo-restrict",
            "not available from your location",
            "blocked it in your country",
        ],
    ),
    (ErrorKind::LiveNotStarted, &["live event will begin", "premieres in", "this live event will", "is_upcoming"]),
    (ErrorKind::RateLimited, &["http error 429", "too many requests", "rate-limit", "rate limit"]),
    (
        ErrorKind::FfmpegMissing,
        &[
            "ffmpeg not found",
            "ffprobe and ffmpeg not found",
            "ffmpeg is not installed",
            "you have requested merging of multiple formats but ffmpeg is not installed",
        ],
    ),
    (
        ErrorKind::FormatUnavailable,
        &["requested format is not available", "requested format not available", "no video formats found"],
    ),
    (ErrorKind::DiskFull, &["no space left on device", "errno 28", "disk full", "there is not enough space"]),
    (ErrorKind::PermissionDenied, &["permission denied", "errno 13", "access is denied", "winerror 5"]),
    (
        ErrorKind::InvalidPath,
        &["no such file or directory", "errno 2]", "invalid argument", "winerror 123", "filename too long", "errno 36"],
    ),
    (ErrorKind::Timeout, &["timed out", "timeout", "read operation timed out"]),
    (
        ErrorKind::Network,
        &[
            "unable to download webpage",
            "getaddrinfo failed",
            "name or service not known",
            "temporary failure in name resolution",
            "connection refused",
            "connection reset",
            "network is unreachable",
            "nodename nor servname",
            "failed to resolve",
            "ssl:",
            "remote end closed connection",
            "http error 5",
        ],
    ),
    (
        ErrorKind::Unavailable,
        &[
            "video unavailable",
            "this video is unavailable",
            "has been removed",
            "no longer available",
            "http error 404",
            "does not exist",
            "this video has been removed",
            "account associated with this video has been terminated",
        ],
    ),
    (
        ErrorKind::ConversionFailed,
        &["postprocessing:", "conversion failed", "error opening output file", "ffmpeg exited with code"],
    ),
    (
        ErrorKind::ExtractorOutdated,
        &[
            "unable to extract",
            "please report this issue",
            "update to the latest version",
            "confirm you are on the latest version",
            "unable to parse",
        ],
    ),
];

/// Classify yt-dlp output into an [`ErrorKind`].
pub fn classify(output: &str) -> ErrorKind {
    let lower = output.to_ascii_lowercase();
    // Prefer explicit ERROR lines; fall back to the whole output.
    let error_lines: String = lower.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join("\n");
    for haystack in [&error_lines, &lower] {
        if haystack.is_empty() {
            continue;
        }
        for (kind, needles) in RULES {
            if needles.iter().any(|n| haystack.contains(n)) {
                return *kind;
            }
        }
    }
    ErrorKind::Unknown
}

/// Build a report from yt-dlp's stderr/stdout.
pub fn report_from_output(output: &str) -> FailureReport {
    let kind = classify(output);
    let summary = output
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("ERROR:").map(|s| s.trim().to_string()))
        .or_else(|| output.lines().rev().find(|l| !l.trim().is_empty()).map(|s| s.trim().to_string()))
        .unwrap_or_else(|| kind.title().to_string());
    FailureReport::new(kind, summary, output)
}

/// Remove things that look like secrets from diagnostic output: cookie
/// headers, authorization headers and credentials in proxy URLs.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("cookie:") || lower.contains("authorization:") || lower.contains("set-cookie") {
            out.push_str("[redacted header]\n");
            continue;
        }
        out.push_str(&redact_userinfo(line));
        out.push('\n');
    }
    if !s.ends_with('\n') {
        out.pop();
    }
    out
}

fn redact_userinfo(line: &str) -> String {
    // scheme://user:pass@host → scheme://***@host
    let mut result = String::new();
    let mut rest = line;
    while let Some(pos) = rest.find("://") {
        let (before, after) = rest.split_at(pos + 3);
        result.push_str(before);
        let end = after.find(|c: char| c == '/' || c.is_whitespace()).unwrap_or(after.len());
        let authority = &after[..end];
        if let Some(at) = authority.rfind('@') {
            result.push_str("***");
            result.push_str(&authority[at..]);
        } else {
            result.push_str(authority);
        }
        rest = &after[end..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_errors() {
        let cases = [
            ("ERROR: Unsupported URL: https://example.com/", ErrorKind::UnsupportedUrl),
            ("ERROR: [youtube] abc: Private video. Sign in if you've been granted access", ErrorKind::Private),
            ("ERROR: [youtube] abc: Video unavailable", ErrorKind::Unavailable),
            ("ERROR: [youtube] abc: Sign in to confirm you're not a bot. Use --cookies", ErrorKind::LoginRequired),
            ("ERROR: [youtube] abc: Sign in to confirm your age", ErrorKind::AgeRestricted),
            ("ERROR: The uploader has not made this video available in your country", ErrorKind::GeoRestricted),
            ("ERROR: [x] abc: This video is not available in your country", ErrorKind::GeoRestricted),
            ("ERROR: unable to download video data: HTTP Error 429: Too Many Requests", ErrorKind::RateLimited),
            (
                "ERROR: You have requested merging of multiple formats but ffmpeg is not installed",
                ErrorKind::FfmpegMissing,
            ),
            ("ERROR: [youtube] abc: Requested format is not available", ErrorKind::FormatUnavailable),
            ("ERROR: unable to write data: [Errno 28] No space left on device", ErrorKind::DiskFull),
            ("ERROR: unable to open for writing: [Errno 13] Permission denied: '/x'", ErrorKind::PermissionDenied),
            (
                "ERROR: [generic] Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>",
                ErrorKind::Network,
            ),
            ("ERROR: [Errno 110] Connection timed out", ErrorKind::Timeout),
            ("ERROR: [foo] 123: Unable to extract video data; please report this issue", ErrorKind::ExtractorOutdated),
            ("ERROR: Postprocessing: Conversion failed!", ErrorKind::ConversionFailed),
            ("ERROR: [Netflix] This video is DRM protected", ErrorKind::Drm),
            ("something odd", ErrorKind::Unknown),
        ];
        for (input, expected) in cases {
            assert_eq!(classify(input), expected, "{input}");
        }
    }

    #[test]
    fn report_summary_uses_last_error_line() {
        let out = "WARNING: foo\nERROR: first\n[info] x\nERROR: [youtube] abc: Video unavailable\n";
        let r = report_from_output(out);
        assert_eq!(r.kind, ErrorKind::Unavailable);
        assert_eq!(r.summary, "[youtube] abc: Video unavailable");
    }

    #[test]
    fn transient_kinds() {
        assert!(ErrorKind::Network.is_transient());
        assert!(!ErrorKind::Private.is_transient());
        assert!(!ErrorKind::Drm.is_transient());
    }

    #[test]
    fn redacts_secrets() {
        let s = "Cookie: SID=abc\nproxy http://user:secret@proxy.example:8080 used\nok";
        let r = redact(s);
        assert!(!r.contains("SID=abc"));
        assert!(!r.contains("secret"));
        assert!(r.contains("http://***@proxy.example:8080"));
        assert!(r.ends_with("ok"));
    }
}
