//! User-facing download options. These are serialisable so they can be stored
//! in presets and in the settings file.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadMode {
    /// Best video + best audio, merged when served separately.
    #[default]
    VideoAndAudio,
    /// Video stream only.
    VideoOnly,
    /// Extract audio.
    AudioOnly,
    /// The best single file exactly as the site serves it, no post-processing.
    Original,
}

impl DownloadMode {
    pub const ALL: [DownloadMode; 4] =
        [DownloadMode::VideoAndAudio, DownloadMode::AudioOnly, DownloadMode::VideoOnly, DownloadMode::Original];

    pub fn label(self) -> &'static str {
        match self {
            DownloadMode::VideoAndAudio => "Video",
            DownloadMode::VideoOnly => "Video only (no sound)",
            DownloadMode::AudioOnly => "Audio",
            DownloadMode::Original => "Original file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    /// Whatever yt-dlp produces (merges into mkv/webm/mp4 as needed).
    Original,
    #[default]
    Mp4,
    Mkv,
    Webm,
    Mov,
}

impl Container {
    pub const ALL: [Container; 5] =
        [Container::Mp4, Container::Mkv, Container::Webm, Container::Mov, Container::Original];

    pub fn ext(self) -> Option<&'static str> {
        match self {
            Container::Original => None,
            Container::Mp4 => Some("mp4"),
            Container::Mkv => Some("mkv"),
            Container::Webm => Some("webm"),
            Container::Mov => Some("mov"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Container::Original => "Original",
            Container::Mp4 => "MP4",
            Container::Mkv => "MKV",
            Container::Webm => "WebM",
            Container::Mov => "MOV",
        }
    }

    pub fn supports_embedded_subs(self) -> bool {
        matches!(self, Container::Mp4 | Container::Mkv | Container::Webm | Container::Mov)
    }

    pub fn supports_embedded_thumbnail(self) -> bool {
        matches!(self, Container::Mp4 | Container::Mkv | Container::Mov)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodecPreference {
    #[default]
    Any,
    H264,
    H265,
    Vp9,
    Av1,
}

impl CodecPreference {
    pub const ALL: [CodecPreference; 5] = [
        CodecPreference::Any,
        CodecPreference::H264,
        CodecPreference::H265,
        CodecPreference::Vp9,
        CodecPreference::Av1,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CodecPreference::Any => "Automatic",
            CodecPreference::H264 => "H.264 (most compatible)",
            CodecPreference::H265 => "H.265 / HEVC",
            CodecPreference::Vp9 => "VP9",
            CodecPreference::Av1 => "AV1 (smallest files)",
        }
    }

    /// yt-dlp format-sort token.
    pub fn sort_token(self) -> Option<&'static str> {
        match self {
            CodecPreference::Any => None,
            CodecPreference::H264 => Some("vcodec:h264"),
            CodecPreference::H265 => Some("vcodec:h265"),
            CodecPreference::Vp9 => Some("vcodec:vp9"),
            CodecPreference::Av1 => Some("vcodec:av01"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FpsPreference {
    #[default]
    Highest,
    UpTo30,
    UpTo60,
}

impl FpsPreference {
    pub const ALL: [FpsPreference; 3] = [FpsPreference::Highest, FpsPreference::UpTo60, FpsPreference::UpTo30];

    pub fn label(self) -> &'static str {
        match self {
            FpsPreference::Highest => "Highest available",
            FpsPreference::UpTo60 => "Up to 60 fps",
            FpsPreference::UpTo30 => "Up to 30 fps",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VideoOptions {
    pub container: Container,
    /// Maximum height in pixels; `None` = best available.
    pub max_height: Option<u32>,
    pub codec: CodecPreference,
    pub fps: FpsPreference,
    /// Allow re-encoding when the chosen streams cannot be remuxed into the
    /// target container. Off by default to avoid quality loss.
    pub allow_recode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// Keep the source codec; only remux into an audio container.
    #[default]
    Original,
    Mp3,
    M4a,
    Flac,
    Wav,
    Opus,
    Ogg,
}

impl AudioFormat {
    pub const ALL: [AudioFormat; 7] = [
        AudioFormat::Mp3,
        AudioFormat::M4a,
        AudioFormat::Opus,
        AudioFormat::Flac,
        AudioFormat::Wav,
        AudioFormat::Ogg,
        AudioFormat::Original,
    ];

    /// Value for `--audio-format`. `None` means "best" (no conversion).
    pub fn yt_dlp_name(self) -> Option<&'static str> {
        match self {
            AudioFormat::Original => None,
            AudioFormat::Mp3 => Some("mp3"),
            AudioFormat::M4a => Some("m4a"),
            AudioFormat::Flac => Some("flac"),
            AudioFormat::Wav => Some("wav"),
            AudioFormat::Opus => Some("opus"),
            AudioFormat::Ogg => Some("vorbis"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AudioFormat::Original => "Original (no conversion)",
            AudioFormat::Mp3 => "MP3",
            AudioFormat::M4a => "M4A (AAC)",
            AudioFormat::Flac => "FLAC (lossless container)",
            AudioFormat::Wav => "WAV (uncompressed)",
            AudioFormat::Opus => "Opus",
            AudioFormat::Ogg => "OGG (Vorbis)",
        }
    }

    /// Lossless targets ignore bitrate settings.
    pub fn is_lossless(self) -> bool {
        matches!(self, AudioFormat::Flac | AudioFormat::Wav)
    }

    pub fn supports_cover_art(self) -> bool {
        !matches!(self, AudioFormat::Wav)
    }

    /// Source codec to prefer so that conversion can be avoided.
    fn preferred_source(self) -> Option<&'static str> {
        match self {
            AudioFormat::M4a => Some("acodec:aac"),
            AudioFormat::Opus => Some("acodec:opus"),
            AudioFormat::Mp3 => Some("acodec:mp3"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum AudioQuality {
    /// Best VBR quality of the encoder (yt-dlp `--audio-quality 0`).
    #[default]
    Best,
    /// Target bitrate in kbps for lossy encoders.
    Kbps(u32),
}

impl AudioQuality {
    pub const CHOICES: [AudioQuality; 5] = [
        AudioQuality::Best,
        AudioQuality::Kbps(320),
        AudioQuality::Kbps(256),
        AudioQuality::Kbps(192),
        AudioQuality::Kbps(128),
    ];

    pub fn label(self) -> String {
        match self {
            AudioQuality::Best => "Best (highest-quality VBR)".into(),
            AudioQuality::Kbps(k) => format!("{k} kbps target"),
        }
    }

    pub fn yt_dlp_value(self) -> String {
        match self {
            AudioQuality::Best => "0".into(),
            AudioQuality::Kbps(k) => format!("{k}K"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AudioOptions {
    pub format: AudioFormat,
    pub quality: AudioQuality,
    /// EBU R128 loudness normalisation (requires conversion).
    pub normalize: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleOptions {
    pub download: bool,
    pub embed: bool,
    /// yt-dlp language patterns, e.g. `en`, `en.*`, `all`.
    pub languages: Vec<String>,
    pub include_auto_generated: bool,
    /// Convert to SRT for broad player compatibility.
    pub convert_to_srt: bool,
}

impl Default for SubtitleOptions {
    fn default() -> Self {
        Self {
            download: false,
            embed: true,
            languages: vec!["en".into()],
            include_auto_generated: false,
            convert_to_srt: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlaylistOptions {
    /// `--playlist-items` spec, e.g. `1,3,5-7`. `None` = all.
    pub items: Option<String>,
    /// Prefix files with their playlist position.
    pub number_files: bool,
    /// Put files in a sub-folder named after the playlist.
    pub subfolder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverwritePolicy {
    /// Never overwrite existing files.
    #[default]
    Skip,
    /// Overwrite existing files.
    Overwrite,
}

/// A specific format picked from the analysed format list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactFormat {
    pub format_id: String,
    /// When true and the mode needs audio, best audio is merged in.
    pub video_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadOptions {
    pub mode: DownloadMode,
    pub video: VideoOptions,
    pub audio: AudioOptions,
    pub subtitles: SubtitleOptions,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub embed_chapters: bool,
    /// Advanced: download this exact format instead of using rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_format: Option<ExactFormat>,
    pub playlist: PlaylistOptions,
    pub overwrite: OverwritePolicy,
    pub restrict_filenames: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            mode: DownloadMode::VideoAndAudio,
            video: VideoOptions::default(),
            audio: AudioOptions::default(),
            subtitles: SubtitleOptions::default(),
            embed_metadata: true,
            embed_thumbnail: true,
            embed_chapters: true,
            exact_format: None,
            playlist: PlaylistOptions::default(),
            overwrite: OverwritePolicy::Skip,
            restrict_filenames: false,
        }
    }
}

impl DownloadOptions {
    /// Whether FFmpeg is required for these options.
    pub fn needs_ffmpeg(&self) -> bool {
        match self.mode {
            DownloadMode::Original => false,
            DownloadMode::AudioOnly => true,
            DownloadMode::VideoOnly => self.video.container != Container::Original || self.embed_any(),
            DownloadMode::VideoAndAudio => true,
        }
    }

    fn embed_any(&self) -> bool {
        self.embed_metadata
            || self.embed_thumbnail
            || self.embed_chapters
            || (self.subtitles.download && self.subtitles.embed)
    }

    /// The target file extension when it is known in advance.
    pub fn expected_extension(&self) -> Option<&'static str> {
        match self.mode {
            DownloadMode::AudioOnly => match self.audio.format {
                AudioFormat::Ogg => Some("ogg"),
                f => f.yt_dlp_name(),
            },
            DownloadMode::VideoAndAudio | DownloadMode::VideoOnly => self.video.container.ext(),
            DownloadMode::Original => None,
        }
    }

    /// Short human summary, e.g. `MP4 · 1080p` or `MP3 · 320 kbps`.
    pub fn summary(&self) -> String {
        match self.mode {
            DownloadMode::AudioOnly => {
                let f = match self.audio.format {
                    AudioFormat::Original => "Audio (original)".to_string(),
                    other => other.label().split(' ').next().unwrap_or("").to_string(),
                };
                if self.audio.format.is_lossless() || self.audio.format == AudioFormat::Original {
                    f
                } else {
                    match self.audio.quality {
                        AudioQuality::Best => format!("{f} · best"),
                        AudioQuality::Kbps(k) => format!("{f} · {k} kbps"),
                    }
                }
            }
            DownloadMode::Original => "Original file".into(),
            DownloadMode::VideoAndAudio | DownloadMode::VideoOnly => {
                let res = self.video.max_height.map(|h| format!("{h}p")).unwrap_or_else(|| "best".into());
                let suffix = if self.mode == DownloadMode::VideoOnly { " · no audio" } else { "" };
                format!("{} · {res}{suffix}", self.video.container.label())
            }
        }
    }

    pub(crate) fn audio_sort_token(&self) -> Option<&'static str> {
        self.audio.format.preferred_source()
    }
}
