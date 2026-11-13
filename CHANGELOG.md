# Changelog

All notable changes to waydlp desktop are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-11-13

### Added
- Optional automatic updates for the waydlp-managed yt-dlp (Settings » Privacy, off by default). It checks GitHub at most once a day and never touches system or custom copies.
- Warning on Home and Downloader when the yt-dlp in use is more than 90 days old, with a one-click update. The check works offline. `waydlp-cli tools` prints the same warning.
- "Retry failed" in the queue retries every failed download with a fresh retry budget.
- `Installers` GitHub Actions workflow: builds the CLI installer (`waydlp-installer`) and the GUI installer (`waydlp-setup`, plus the offline Windows `setup.exe`) for Windows and Linux on x64 and arm64. It tests them on the runners (install, verify, uninstall cycle and a GUI self-test) and attaches them to draft releases.
- `waydlp-setup --self-test`: opens the wizard window, renders a few frames and exits (used by CI).

### Fixed
- Starting the app no longer marks downloads that the CLI or another waydlp window is still running as "Interrupted" in history. Only downloads whose process has exited are marked.
- Analysing a video no longer permanently erases your preferred subtitle languages (one video without them made every later download fall back to English).
- HTTP 403 errors from an outdated yt-dlp are reported as "yt-dlp may be out of date" and are no longer retried pointlessly.
- Links with punctuation from the surrounding text, such as `(https://youtu.be/…).`, are cleaned up; balanced brackets such as Wikipedia's `Foo_(bar)` are kept.
- A stale automatic-retry timer could restart a job early after pause, resume and another failure.
- Queue integration tests no longer time out on busy machines.
- The GUI installer now starts with wgpu and falls back to OpenGL, like the app, so it works on machines without OpenGL 2.0+ drivers.

## [0.1.0] - 2026-11-04

First development version (not released yet).

### Added
- Rust workspace: core logic, download engine, settings, history, platform, theme, GUI, CLI and installers.
- URL analysis with real yt-dlp metadata (title, creator, duration, thumbnail, formats, subtitles) and playlist previews with item selection.
- Video (MP4/MKV/WebM/MOV/original) and audio (MP3/M4A/Opus/FLAC/WAV/OGG/original) downloads; quality, codec and frame-rate preferences; remux-first with optional re-encoding; loudness normalisation; subtitle, thumbnail, chapter and metadata embedding.
- Download queue with real progress, concurrency limit, pause/resume (partial data kept), cancel (process tree killed, partial data removed), automatic retries for transient errors, reordering.
- Readable error classification with expandable technical details (secrets redacted).
- SQLite history with search/filters, re-download, open/reveal, remove entry, delete files with confirmation.
- Presets (built-in and user), versioned settings with migrations, import/export, portable mode.
- Verified installation of managed yt-dlp and (Windows) FFmpeg; update check for waydlp with install-kind specific advice.
- `waydlp-cli`, `waydlp-installer` (CLI) and `waydlp-setup` (GUI installer, also the offline Windows setup.exe).
- Packaging for Windows (setup.exe, MSI, portable zip) and Linux (tar.gz, deb, rpm, AppImage, Flatpak manifest, Arch PKGBUILDs).
- CI (fmt, clippy, tests on Linux/Windows, cargo-deny, package smoke tests) and a draft-release workflow.
