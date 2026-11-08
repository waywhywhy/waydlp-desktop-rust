//! Built-in and user-defined download presets.

use crate::options::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    pub options: DownloadOptions,
}

impl Preset {
    fn builtin(id: &str, name: &str, options: DownloadOptions) -> Self {
        Self { id: id.into(), name: name.into(), builtin: true, options }
    }
}

/// The presets that ship with waydlp. They cannot be deleted.
pub fn builtin_presets() -> Vec<Preset> {
    // MP4 presets prefer H.264 because it plays on virtually every device;
    // "Best Video Quality" lets yt-dlp pick the best codec (often AV1/VP9).
    let video = |container: Container, max_height: Option<u32>| DownloadOptions {
        mode: DownloadMode::VideoAndAudio,
        video: VideoOptions {
            container,
            max_height,
            codec: if container == Container::Mp4 { CodecPreference::H264 } else { CodecPreference::Any },
            ..Default::default()
        },
        ..Default::default()
    };
    let audio = |format: AudioFormat, quality: AudioQuality| DownloadOptions {
        mode: DownloadMode::AudioOnly,
        audio: AudioOptions { format, quality, normalize: false },
        ..Default::default()
    };
    vec![
        Preset::builtin("best-video", "Best Video Quality", video(Container::Mkv, None)),
        Preset::builtin("best-audio", "Best Audio Quality", audio(AudioFormat::Original, AudioQuality::Best)),
        Preset::builtin("mp4-1080", "MP4 1080p", video(Container::Mp4, Some(1080))),
        Preset::builtin("mp4-720", "MP4 720p", video(Container::Mp4, Some(720))),
        Preset::builtin("mp3", "MP3", audio(AudioFormat::Mp3, AudioQuality::Kbps(320))),
        Preset::builtin(
            "original",
            "Original Format",
            DownloadOptions {
                mode: DownloadMode::Original,
                embed_metadata: false,
                embed_thumbnail: false,
                embed_chapters: false,
                ..Default::default()
            },
        ),
    ]
}

/// Create a fresh, URL-safe id for a user preset from its name.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "preset".into() } else { format!("user-{out}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_unique_ids() {
        let p = builtin_presets();
        let mut ids: Vec<_> = p.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), p.len());
        assert!(p.iter().all(|p| p.builtin));
    }

    #[test]
    fn presets_roundtrip_toml_like_json() {
        for p in builtin_presets() {
            let s = serde_json::to_string(&p).unwrap();
            let back: Preset = serde_json::from_str(&s).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn slug() {
        assert_eq!(slugify("My  Podcast MP3!"), "user-my-podcast-mp3");
        assert_eq!(slugify("***"), "preset");
    }
}
