//! waydlp download engine.
//!
//! This crate is UI-agnostic: the GUI and the CLI both drive it.
//!
//! - [`tools`]: locate yt-dlp / FFmpeg / FFprobe and read their versions.
//! - [`run`]: probe URLs and run downloads with real, parsed progress.
//! - [`queue`]: concurrent download queue with pause/resume/cancel/retry.
//! - [`managed`]: verified installation of managed yt-dlp / FFmpeg copies.
//! - [`updates`]: check GitHub Releases for new waydlp versions.

pub mod managed;
pub mod process;
pub mod queue;
pub mod run;
pub mod tools;
pub mod updates;

pub use queue::{JobId, JobSnapshot, JobSpec, JobState, QueueEvent, QueueHandle, QueueSettings, spawn_queue};
pub use tools::{ToolInfo, ToolSearch, ToolSource, ToolStatus, detect_tools};

/// HTTP client used for every request waydlp itself makes (update checks,
/// managed tool downloads). Certificates are always validated.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!(
            "waydlp/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/waywhywhy/waydlp-desktop-rust)"
        ))
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("TLS backend available")
}
