# Changelog

All notable changes to waydlp desktop are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `Installers` GitHub Actions workflow: builds the CLI installer (`waydlp-installer`) and the GUI installer (`waydlp-setup`, plus the offline Windows `setup.exe`) for Windows and Linux on x64 and arm64. It tests them on the runners (install, verify, uninstall cycle and a GUI self-test) and attaches them to draft releases.
- `waydlp-setup --self-test`: opens the wizard window, renders a few frames and exits (used by CI).

### Fixed
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
