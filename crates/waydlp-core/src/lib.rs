//! Pure, I/O-free domain logic for waydlp.
//!
//! Everything in this crate is deterministic and unit-tested: URL validation,
//! yt-dlp metadata parsing, format analysis, download option modelling,
//! yt-dlp argument generation, progress-line parsing, error classification,
//! filename sanitisation and release-artifact naming.
//!
//! Process management and networking live in `waydlp-engine`.
#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

pub mod args;
pub mod artifacts;
pub mod custom_args;
pub mod errors;
pub mod humanize;
pub mod media;
pub mod options;
pub mod presets;
pub mod progress;
pub mod sanitize;
pub mod site;
pub mod urls;

pub use errors::{ErrorKind, FailureReport};
pub use media::{FormatInfo, MediaInfo, PlaylistEntry, PlaylistInfo, ProbeResult};
pub use options::{
    AudioFormat, AudioOptions, AudioQuality, CodecPreference, Container, DownloadMode, DownloadOptions, FpsPreference,
    OverwritePolicy, PlaylistOptions, SubtitleOptions, VideoOptions,
};
pub use presets::Preset;
pub use progress::ProgressEvent;

/// The application name, used consistently everywhere.
pub const APP_NAME: &str = "waydlp";

/// The crate version of the waydlp workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
