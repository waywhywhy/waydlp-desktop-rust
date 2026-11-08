//! Generation of yt-dlp command-line arguments.
//!
//! The output is a `Vec<String>` that is handed to the OS as argv directly —
//! no shell is ever involved, so untrusted input cannot inject commands. The
//! URL is always placed after `--` so it can never be parsed as an option.

use crate::custom_args::{CustomArgsError, parse_custom_args};
use crate::options::*;
use crate::progress;
use crate::sanitize::{DEFAULT_TEMPLATE, TemplateError, validate_output_template};
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

/// Network-related options from the settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkOptions {
    /// e.g. `2M`, `500K`; `None` = unlimited.
    pub rate_limit: Option<String>,
    pub retries: u32,
    pub fragment_retries: u32,
    pub socket_timeout_secs: Option<u32>,
    pub proxy: Option<String>,
}

/// Environment-specific paths resolved by the engine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineContext {
    pub output_dir: PathBuf,
    pub temp_dir: Option<PathBuf>,
    pub ffmpeg_location: Option<PathBuf>,
    pub archive_file: Option<PathBuf>,
    pub cookies_file: Option<PathBuf>,
    /// Respect the user's own yt-dlp config files (off by default so that
    /// waydlp behaves predictably).
    pub use_yt_dlp_config: bool,
}

/// One download request: what to fetch and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub url: Url,
    pub options: DownloadOptions,
    /// `None` uses [`DEFAULT_TEMPLATE`].
    pub filename_template: Option<String>,
    /// Treat the URL as a playlist (`--yes-playlist`) or single item (`--no-playlist`).
    pub is_playlist: bool,
    /// Raw extra arguments from advanced settings.
    pub custom_args: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArgsError {
    #[error(transparent)]
    Template(#[from] TemplateError),
    #[error(transparent)]
    CustomArgs(#[from] CustomArgsError),
    #[error("Invalid download speed limit \"{0}\". Use a number with K, M or G, e.g. 2M.")]
    RateLimit(String),
    #[error("Invalid proxy address \"{0}\". Use e.g. http://host:port or socks5://host:port.")]
    Proxy(String),
    #[error("Invalid playlist selection \"{0}\". Use e.g. 1,3,5-7.")]
    PlaylistItems(String),
    #[error("Invalid format id \"{0}\".")]
    FormatId(String),
    #[error(
        "Loudness normalisation re-encodes the audio. Choose an output format such as MP3 or \
         M4A instead of \"Original\"."
    )]
    NormalizeNeedsConversion,
    #[error("Invalid subtitle language \"{0}\".")]
    SubtitleLanguage(String),
}

pub fn validate_rate_limit(s: &str) -> Result<(), ArgsError> {
    let num = s.strip_suffix(['K', 'M', 'G', 'k', 'm', 'g']).unwrap_or(s);
    let ok = !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit() || c == '.')
        && num.chars().filter(|c| *c == '.').count() <= 1
        && num.chars().any(|c| c.is_ascii_digit());
    if ok { Ok(()) } else { Err(ArgsError::RateLimit(s.to_string())) }
}

pub fn validate_proxy(s: &str) -> Result<(), ArgsError> {
    let u = Url::parse(s).map_err(|_| ArgsError::Proxy(s.to_string()))?;
    let scheme_ok = matches!(u.scheme(), "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h");
    if scheme_ok && u.host_str().is_some_and(|h| !h.is_empty()) { Ok(()) } else { Err(ArgsError::Proxy(s.to_string())) }
}

pub fn validate_playlist_items(s: &str) -> Result<(), ArgsError> {
    let t = s.trim();
    let ok = !t.is_empty()
        && t.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | ':' | ' '))
        && t.chars().any(|c| c.is_ascii_digit());
    if ok { Ok(()) } else { Err(ArgsError::PlaylistItems(s.to_string())) }
}

fn validate_format_id(s: &str) -> Result<(), ArgsError> {
    let ok =
        !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok { Ok(()) } else { Err(ArgsError::FormatId(s.to_string())) }
}

fn validate_sub_lang(s: &str) -> Result<(), ArgsError> {
    let ok = !s.is_empty()
        && s.len() <= 32
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '*' | '!'));
    if ok { Ok(()) } else { Err(ArgsError::SubtitleLanguage(s.to_string())) }
}

/// Arguments common to every yt-dlp invocation.
fn base_args(ctx: &EngineContext, net: &NetworkOptions) -> Result<Vec<String>, ArgsError> {
    let mut a: Vec<String> = Vec::new();
    if !ctx.use_yt_dlp_config {
        a.push("--ignore-config".into());
    }
    a.extend(["--color".into(), "never".into(), "--no-cache-dir".into()]);
    if let Some(rate) = &net.rate_limit {
        let rate = rate.trim();
        if !rate.is_empty() {
            validate_rate_limit(rate)?;
            a.extend(["--limit-rate".into(), rate.to_string()]);
        }
    }
    a.extend(["--retries".into(), net.retries.to_string()]);
    a.extend(["--fragment-retries".into(), net.fragment_retries.to_string()]);
    if let Some(t) = net.socket_timeout_secs {
        a.extend(["--socket-timeout".into(), t.max(1).to_string()]);
    }
    if let Some(proxy) = &net.proxy {
        let proxy = proxy.trim();
        if !proxy.is_empty() {
            validate_proxy(proxy)?;
            a.extend(["--proxy".into(), proxy.to_string()]);
        }
    }
    if let Some(c) = &ctx.cookies_file {
        a.extend(["--cookies".into(), c.display().to_string()]);
    }
    Ok(a)
}

/// Arguments for probing a URL (`-J`). Playlists are listed flat so that a
/// channel with thousands of videos returns quickly and nothing is
/// downloaded.
pub fn build_probe_args(
    url: &Url,
    ctx: &EngineContext,
    net: &NetworkOptions,
    no_playlist: bool,
) -> Result<Vec<String>, ArgsError> {
    let mut a = base_args(ctx, net)?;
    a.extend(["--dump-single-json".into(), "--flat-playlist".into(), "--no-warnings".into()]);
    if no_playlist {
        a.push("--no-playlist".into());
    }
    a.push("--".into());
    a.push(url.as_str().into());
    Ok(a)
}

/// Build the full download argument list.
pub fn build_download_args(
    req: &DownloadRequest,
    ctx: &EngineContext,
    net: &NetworkOptions,
) -> Result<Vec<String>, ArgsError> {
    let o = &req.options;
    let mut a = base_args(ctx, net)?;

    // Machine-readable progress.
    a.extend([
        "--newline".into(),
        "--progress".into(),
        "--no-simulate".into(),
        "--progress-template".into(),
        progress::DOWNLOAD_TEMPLATE.into(),
        "--progress-template".into(),
        progress::POSTPROCESS_TEMPLATE.into(),
        "--print".into(),
        progress::ITEM_PRINT.into(),
        "--print".into(),
        progress::FILE_PRINT.into(),
    ]);

    if let Some(ff) = &ctx.ffmpeg_location {
        a.extend(["--ffmpeg-location".into(), ff.display().to_string()]);
    }

    // Output location.
    a.extend(["--paths".into(), format!("home:{}", ctx.output_dir.display())]);
    if let Some(t) = &ctx.temp_dir {
        a.extend(["--paths".into(), format!("temp:{}", t.display())]);
    }
    let base_template = req.filename_template.as_deref().unwrap_or(DEFAULT_TEMPLATE).trim();
    validate_output_template(base_template)?;
    let mut template = base_template.to_string();
    if req.is_playlist {
        if o.playlist.number_files {
            // Insert the index in front of the file name part only.
            match template.rfind(['/', '\\']) {
                Some(i) => template.insert_str(i + 1, "%(playlist_index)03d - "),
                None => template.insert_str(0, "%(playlist_index)03d - "),
            }
        }
        if o.playlist.subfolder {
            template = format!("%(playlist_title,playlist_id|Playlist)s/{template}");
        }
    }
    a.extend(["--output".into(), template]);

    // Playlist handling.
    if req.is_playlist {
        a.push("--yes-playlist".into());
        if let Some(items) = &o.playlist.items {
            let items = items.trim();
            if !items.is_empty() {
                validate_playlist_items(items)?;
                a.extend(["--playlist-items".into(), items.replace(' ', "")]);
            }
        }
    } else {
        a.push("--no-playlist".into());
    }
    if let Some(archive) = &ctx.archive_file {
        a.extend(["--download-archive".into(), archive.display().to_string()]);
    }

    match o.overwrite {
        OverwritePolicy::Skip => a.push("--no-overwrites".into()),
        OverwritePolicy::Overwrite => a.push("--force-overwrites".into()),
    }
    if o.restrict_filenames {
        a.push("--restrict-filenames".into());
    }

    // Format selection and post-processing.
    match o.mode {
        DownloadMode::Original => {
            let f = match &o.exact_format {
                Some(ex) => {
                    validate_format_id(&ex.format_id)?;
                    ex.format_id.clone()
                }
                None => "b/bv*+ba".into(),
            };
            a.extend(["--format".into(), f]);
        }
        DownloadMode::AudioOnly => {
            let f = match &o.exact_format {
                Some(ex) => {
                    validate_format_id(&ex.format_id)?;
                    format!("{}/ba/b", ex.format_id)
                }
                None => "ba/b".into(),
            };
            a.extend(["--format".into(), f]);
            if let Some(tok) = o.audio_sort_token() {
                a.extend(["--format-sort".into(), tok.into()]);
            }
            a.push("--extract-audio".into());
            if let Some(fmt) = o.audio.format.yt_dlp_name() {
                a.extend(["--audio-format".into(), fmt.into()]);
            }
            if !o.audio.format.is_lossless() {
                a.extend(["--audio-quality".into(), o.audio.quality.yt_dlp_value()]);
            }
            if o.audio.normalize {
                if o.audio.format == AudioFormat::Original {
                    return Err(ArgsError::NormalizeNeedsConversion);
                }
                a.extend(["--postprocessor-args".into(), format!("ExtractAudio:{}", normalize_filter_args())]);
            }
        }
        DownloadMode::VideoAndAudio | DownloadMode::VideoOnly => {
            let with_audio = o.mode == DownloadMode::VideoAndAudio;
            let f = video_format_selector(o, with_audio)?;
            a.extend(["--format".into(), f]);
            let sort = video_format_sort(o);
            if !sort.is_empty() {
                a.extend(["--format-sort".into(), sort.join(",")]);
            }
            if let Some(ext) = o.video.container.ext() {
                if with_audio {
                    a.extend(["--merge-output-format".into(), ext.into()]);
                }
                if o.video.allow_recode {
                    a.extend(["--recode-video".into(), ext.into()]);
                } else {
                    a.extend(["--remux-video".into(), ext.into()]);
                }
            }
        }
    }

    // Embedding.
    if o.mode != DownloadMode::Original {
        if o.embed_metadata {
            a.push("--embed-metadata".into());
        }
        let thumb_ok = match o.mode {
            DownloadMode::AudioOnly => o.audio.format.supports_cover_art(),
            _ => o.video.container.supports_embedded_thumbnail() || o.video.container == Container::Original,
        };
        if o.embed_thumbnail && thumb_ok {
            a.push("--embed-thumbnail".into());
        }
        if o.embed_chapters && o.mode != DownloadMode::AudioOnly {
            a.push("--embed-chapters".into());
        }
    }

    // Subtitles.
    if o.subtitles.download && o.mode != DownloadMode::AudioOnly {
        let langs: Vec<String> =
            o.subtitles.languages.iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        for l in &langs {
            validate_sub_lang(l)?;
        }
        a.push("--write-subs".into());
        if o.subtitles.include_auto_generated {
            a.push("--write-auto-subs".into());
        }
        let langs = if langs.is_empty() { "en".to_string() } else { langs.join(",") };
        a.extend(["--sub-langs".into(), langs]);
        if o.subtitles.convert_to_srt {
            a.extend(["--convert-subs".into(), "srt".into()]);
        }
        if o.subtitles.embed && o.video.container.supports_embedded_subs() {
            a.push("--embed-subs".into());
        }
    }

    a.extend(parse_custom_args(&req.custom_args)?);

    a.push("--".into());
    a.push(req.url.as_str().into());
    Ok(a)
}

/// EBU R128 loudness normalisation for FFmpeg.
pub fn normalize_filter_args() -> &'static str {
    "-af loudnorm=I=-16:TP=-1.5:LRA=11"
}

fn video_format_selector(o: &DownloadOptions, with_audio: bool) -> Result<String, ArgsError> {
    if let Some(ex) = &o.exact_format {
        validate_format_id(&ex.format_id)?;
        return Ok(if with_audio && ex.video_only {
            format!("{id}+ba/{id}", id = ex.format_id)
        } else {
            ex.format_id.clone()
        });
    }
    let h = o.video.max_height.map(|h| format!("[height<={h}]")).unwrap_or_default();
    let fps = match o.video.fps {
        FpsPreference::Highest => String::new(),
        FpsPreference::UpTo60 => "[fps<=60]".into(),
        FpsPreference::UpTo30 => "[fps<=30]".into(),
    };
    let filt = format!("{h}{fps}");
    Ok(if with_audio {
        if filt.is_empty() {
            "bv*+ba/b".into()
        } else {
            // Strict choice first; fall back to the best available when the
            // site doesn't report heights/fps (filters would match nothing).
            format!("bv*{filt}+ba/b{filt}/bv*+ba/b")
        }
    } else if filt.is_empty() {
        "bv/bv*".into()
    } else {
        format!("bv{filt}/bv/bv*")
    })
}

fn video_format_sort(o: &DownloadOptions) -> Vec<String> {
    let mut s = Vec::new();
    if let Some(h) = o.video.max_height {
        s.push(format!("res:{h}"));
    }
    match o.video.fps {
        FpsPreference::Highest => {}
        FpsPreference::UpTo60 => s.push("fps:60".into()),
        FpsPreference::UpTo30 => s.push("fps:30".into()),
    }
    if let Some(tok) = o.video.codec.sort_token() {
        s.push(tok.into());
    }
    // Prefer streams that fit the container natively, so we remux instead of
    // transcoding wherever possible.
    match o.video.container {
        Container::Mp4 | Container::Mov => s.push("ext:mp4:m4a".into()),
        Container::Webm => s.push("ext:webm:webm".into()),
        Container::Mkv | Container::Original => {}
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(options: DownloadOptions) -> DownloadRequest {
        DownloadRequest {
            url: Url::parse("https://www.youtube.com/watch?v=abc").unwrap(),
            options,
            filename_template: None,
            is_playlist: false,
            custom_args: String::new(),
        }
    }

    fn ctx() -> EngineContext {
        EngineContext { output_dir: PathBuf::from("/home/u/Videos"), ..Default::default() }
    }

    fn net() -> NetworkOptions {
        NetworkOptions { retries: 10, fragment_retries: 10, ..Default::default() }
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter().position(|a| a == flag).map(|i| args[i + 1].as_str())
    }

    fn values_after<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
        args.iter().enumerate().filter(|(_, a)| *a == flag).map(|(i, _)| args[i + 1].as_str()).collect()
    }

    #[test]
    fn url_is_last_after_double_dash() {
        let a = build_download_args(&req(DownloadOptions::default()), &ctx(), &net()).unwrap();
        let n = a.len();
        assert_eq!(a[n - 2], "--");
        assert_eq!(a[n - 1], "https://www.youtube.com/watch?v=abc");
        assert!(a.contains(&"--ignore-config".to_string()));
        assert!(a.contains(&"--no-playlist".to_string()));
    }

    #[test]
    fn mp4_1080_selects_and_merges() {
        let mut o = DownloadOptions::default();
        o.video = VideoOptions { container: Container::Mp4, max_height: Some(1080), ..Default::default() };
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b"));
        assert_eq!(value_after(&a, "--format-sort"), Some("res:1080,ext:mp4:m4a"));
        assert_eq!(value_after(&a, "--merge-output-format"), Some("mp4"));
        assert_eq!(value_after(&a, "--remux-video"), Some("mp4"));
        assert!(!a.contains(&"--recode-video".to_string()));
        assert!(a.contains(&"--embed-thumbnail".to_string()));
    }

    #[test]
    fn recode_only_when_allowed() {
        let mut o = DownloadOptions::default();
        o.video.allow_recode = true;
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--recode-video"), Some("mp4"));
        assert!(!a.contains(&"--remux-video".to_string()));
    }

    #[test]
    fn codec_and_fps_preferences() {
        let mut o = DownloadOptions::default();
        o.video.codec = CodecPreference::Av1;
        o.video.fps = FpsPreference::UpTo30;
        o.video.container = Container::Mkv;
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("bv*[fps<=30]+ba/b[fps<=30]/bv*+ba/b"));
        assert_eq!(value_after(&a, "--format-sort"), Some("fps:30,vcodec:av01"));
    }

    #[test]
    fn video_only() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::VideoOnly;
        o.video.max_height = Some(720);
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("bv[height<=720]/bv/bv*"));
        assert!(!a.contains(&"--merge-output-format".to_string()));
    }

    #[test]
    fn mp3_320() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::AudioOnly;
        o.audio = AudioOptions { format: AudioFormat::Mp3, quality: AudioQuality::Kbps(320), normalize: false };
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("ba/b"));
        assert!(a.contains(&"--extract-audio".to_string()));
        assert_eq!(value_after(&a, "--audio-format"), Some("mp3"));
        assert_eq!(value_after(&a, "--audio-quality"), Some("320K"));
        assert!(a.contains(&"--embed-thumbnail".to_string()));
        assert!(!a.contains(&"--embed-chapters".to_string()));
        assert!(!a.contains(&"--merge-output-format".to_string()));
    }

    #[test]
    fn lossless_has_no_bitrate_and_wav_no_cover() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::AudioOnly;
        o.audio.format = AudioFormat::Wav;
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--audio-format"), Some("wav"));
        assert!(!a.contains(&"--audio-quality".to_string()));
        assert!(!a.contains(&"--embed-thumbnail".to_string()));
    }

    #[test]
    fn original_audio_keeps_codec() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::AudioOnly;
        o.audio.format = AudioFormat::Original;
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert!(a.contains(&"--extract-audio".to_string()));
        assert!(!a.contains(&"--audio-format".to_string()));
    }

    #[test]
    fn normalization_generates_ffmpeg_filter() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::AudioOnly;
        o.audio = AudioOptions { format: AudioFormat::M4a, quality: AudioQuality::Best, normalize: true };
        let a = build_download_args(&req(o.clone()), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--postprocessor-args"), Some("ExtractAudio:-af loudnorm=I=-16:TP=-1.5:LRA=11"));
        assert_eq!(value_after(&a, "--format-sort"), Some("acodec:aac"));
        o.audio.format = AudioFormat::Original;
        assert_eq!(build_download_args(&req(o), &ctx(), &net()), Err(ArgsError::NormalizeNeedsConversion));
    }

    #[test]
    fn original_mode_has_no_postprocessing() {
        let mut o = DownloadOptions::default();
        o.mode = DownloadMode::Original;
        let a = build_download_args(&req(o), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("b/bv*+ba"));
        for flag in ["--remux-video", "--merge-output-format", "--embed-metadata", "--extract-audio"] {
            assert!(!a.contains(&flag.to_string()), "{flag}");
        }
    }

    #[test]
    fn exact_format_merges_audio_when_video_only() {
        let mut o = DownloadOptions::default();
        o.exact_format = Some(ExactFormat { format_id: "299".into(), video_only: true });
        let a = build_download_args(&req(o.clone()), &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--format"), Some("299+ba/299"));
        o.exact_format = Some(ExactFormat { format_id: "bad id;rm".into(), video_only: false });
        assert!(matches!(build_download_args(&req(o), &ctx(), &net()), Err(ArgsError::FormatId(_))));
    }

    #[test]
    fn subtitles() {
        let mut o = DownloadOptions::default();
        o.subtitles = SubtitleOptions {
            download: true,
            embed: true,
            languages: vec!["en".into(), "de".into()],
            include_auto_generated: true,
            convert_to_srt: true,
        };
        let a = build_download_args(&req(o.clone()), &ctx(), &net()).unwrap();
        assert!(a.contains(&"--write-subs".to_string()));
        assert!(a.contains(&"--write-auto-subs".to_string()));
        assert_eq!(value_after(&a, "--sub-langs"), Some("en,de"));
        assert_eq!(value_after(&a, "--convert-subs"), Some("srt"));
        assert!(a.contains(&"--embed-subs".to_string()));
        o.subtitles.languages = vec!["en;x".into()];
        assert!(build_download_args(&req(o), &ctx(), &net()).is_err());
    }

    #[test]
    fn playlist_template_and_items() {
        let mut o = DownloadOptions::default();
        o.playlist = PlaylistOptions { items: Some("1, 3-5".into()), number_files: true, subfolder: true };
        let mut r = req(o);
        r.is_playlist = true;
        r.filename_template = Some("%(title)s.%(ext)s".into());
        let a = build_download_args(&r, &ctx(), &net()).unwrap();
        assert!(a.contains(&"--yes-playlist".to_string()));
        assert_eq!(value_after(&a, "--playlist-items"), Some("1,3-5"));
        assert_eq!(
            value_after(&a, "--output"),
            Some("%(playlist_title,playlist_id|Playlist)s/%(playlist_index)03d - %(title)s.%(ext)s")
        );
        r.options.playlist.items = Some("1;2".into());
        assert!(build_download_args(&r, &ctx(), &net()).is_err());
    }

    #[test]
    fn numbering_inserted_after_folder() {
        let mut o = DownloadOptions::default();
        o.playlist.number_files = true;
        let mut r = req(o);
        r.is_playlist = true;
        r.filename_template = Some("%(uploader)s/%(title)s.%(ext)s".into());
        let a = build_download_args(&r, &ctx(), &net()).unwrap();
        assert_eq!(value_after(&a, "--output"), Some("%(uploader)s/%(playlist_index)03d - %(title)s.%(ext)s"));
    }

    #[test]
    fn rejects_traversal_template() {
        let mut r = req(DownloadOptions::default());
        r.filename_template = Some("../../x.%(ext)s".into());
        assert!(matches!(
            build_download_args(&r, &ctx(), &net()),
            Err(ArgsError::Template(TemplateError::ParentTraversal))
        ));
    }

    #[test]
    fn network_options() {
        let n = NetworkOptions {
            rate_limit: Some("2.5M".into()),
            retries: 3,
            fragment_retries: 5,
            socket_timeout_secs: Some(20),
            proxy: Some("socks5://127.0.0.1:9050".into()),
        };
        let a = build_download_args(&req(DownloadOptions::default()), &ctx(), &n).unwrap();
        assert_eq!(value_after(&a, "--limit-rate"), Some("2.5M"));
        assert_eq!(value_after(&a, "--retries"), Some("3"));
        assert_eq!(value_after(&a, "--fragment-retries"), Some("5"));
        assert_eq!(value_after(&a, "--socket-timeout"), Some("20"));
        assert_eq!(value_after(&a, "--proxy"), Some("socks5://127.0.0.1:9050"));

        for bad in ["fast", "2X", "M", "1..2M", ""] {
            if bad.is_empty() {
                continue;
            }
            assert!(validate_rate_limit(bad).is_err(), "{bad}");
        }
        for good in ["500K", "2M", "1G", "1048576", "1.5m"] {
            assert!(validate_rate_limit(good).is_ok(), "{good}");
        }
        assert!(validate_proxy("ftp://x").is_err());
        assert!(validate_proxy("not a url").is_err());
    }

    #[test]
    fn context_paths() {
        let c = EngineContext {
            output_dir: PathBuf::from("out"),
            temp_dir: Some(PathBuf::from("tmp")),
            ffmpeg_location: Some(PathBuf::from("bin/ffmpeg")),
            archive_file: Some(PathBuf::from("archive.txt")),
            cookies_file: Some(PathBuf::from("cookies.txt")),
            use_yt_dlp_config: true,
        };
        let a = build_download_args(&req(DownloadOptions::default()), &c, &net()).unwrap();
        assert_eq!(values_after(&a, "--paths"), vec!["home:out", "temp:tmp"]);
        assert!(value_after(&a, "--ffmpeg-location").unwrap().contains("ffmpeg"));
        assert_eq!(value_after(&a, "--download-archive"), Some("archive.txt"));
        assert_eq!(value_after(&a, "--cookies"), Some("cookies.txt"));
        assert!(!a.contains(&"--ignore-config".to_string()));
    }

    #[test]
    fn custom_args_inserted_before_url() {
        let mut r = req(DownloadOptions::default());
        r.custom_args = "--sponsorblock-remove sponsor".into();
        let a = build_download_args(&r, &ctx(), &net()).unwrap();
        let i = a.iter().position(|x| x == "--sponsorblock-remove").unwrap();
        let dd = a.iter().rposition(|x| x == "--").unwrap();
        assert!(i < dd);
        r.custom_args = "--exec calc".into();
        assert!(matches!(build_download_args(&r, &ctx(), &net()), Err(ArgsError::CustomArgs(_))));
    }

    #[test]
    fn probe_args() {
        let u = Url::parse("https://vimeo.com/1").unwrap();
        let a = build_probe_args(&u, &ctx(), &net(), true).unwrap();
        assert!(a.contains(&"--dump-single-json".to_string()));
        assert!(a.contains(&"--flat-playlist".to_string()));
        assert!(a.contains(&"--no-playlist".to_string()));
        assert_eq!(a.last().unwrap(), "https://vimeo.com/1");
    }
}
